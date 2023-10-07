use std::path::{Path, PathBuf};

use glob::glob;
use lsp_types::{Position, Range};
use tree_sitter::{Node, QueryCursor};

use crate::{
    indexer::{ArcIndexer, Indexer},
    m2::{M2Area, M2Item, M2Path},
    queries,
    ts::{self, node_at_position},
};

enum JSTypes {
    Map,
    Paths,
    Mixins,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsCompletionType {
    Definition,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsCompletion {
    pub text: String,
    pub range: Range,
    pub kind: JsCompletionType,
}

pub fn update_index(index: &ArcIndexer, path: &PathBuf) {
    // if current workspace is magento module
    process_glob(index, &path.append(&["view", "*", "requirejs-config.js"]));
    // if current workspace is magento installation
    process_glob(
        index,
        &path.append(&["vendor", "*", "*", "view", "*", "requirejs-config.js"]),
    );
    process_glob(
        index,
        &path.append(&["vendor", "*", "*", "Magento_Theme", "requirejs-config.js"]),
    );
    process_glob(
        index,
        &path.append(&["app", "code", "*", "*", "view", "*", "requirejs-config.js"]),
    );
    process_glob(
        index,
        &path.append(&["app", "design", "**", "requirejs-config.js"]),
    );
}

fn process_glob(index: &ArcIndexer, glob_path: &PathBuf) {
    let modules = glob(glob_path.to_path_str())
        .expect("Failed to read glob pattern")
        .filter_map(Result::ok);

    for file_path in modules {
        let content =
            std::fs::read_to_string(&file_path).expect("Should have been able to read the file");

        update_index_from_config(index, &content, &file_path.get_area());
    }
}

pub fn get_completion_item(content: &str, pos: Position) -> Option<JsCompletion> {
    let tree = tree_sitter_parsers::parse(content, "javascript");
    let query = queries::js_completion_definition_item();
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(query, tree.root_node(), content.as_bytes());

    for m in matches {
        let node = m.captures[1].node;
        if node_at_position(node, pos) {
            let mut text = ts::get_node_text_before_pos(node, content, pos);
            if text.is_empty() {
                return None;
            }
            text = text[1..].to_string();
            let range = Range {
                start: Position {
                    line: node.start_position().row as u32,
                    character: 1 + node.start_position().column as u32,
                },
                end: pos,
            };

            return Some(JsCompletion {
                text,
                range,
                kind: JsCompletionType::Definition,
            });
        }
    }

    None
}

pub fn get_item_from_position(index: &Indexer, path: &PathBuf, pos: Position) -> Option<M2Item> {
    let content = index.get_file(path)?;
    get_item_from_pos(index, content, path, pos)
}

fn get_item_from_pos(index: &Indexer, content: &str, path: &Path, pos: Position) -> Option<M2Item> {
    let tree = tree_sitter_parsers::parse(content, "javascript");
    let query = queries::js_item_from_pos();
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(query, tree.root_node(), content.as_bytes());

    for m in matches {
        if node_at_position(m.captures[1].node, pos) {
            let text = get_node_text(m.captures[1].node, content);
            let text = resolve_component_text(index, text, &path.to_path_buf().get_area())?;
            return text_to_component(index, text, path);
        }
    }

    None
}

pub fn resolve_component_text<'a>(
    index: &'a Indexer,
    text: &'a str,
    area: &M2Area,
) -> Option<&'a str> {
    index.get_component_map(text, area).map_or_else(
        || {
            area.lower_area()
                .map_or_else(|| Some(text), |a| resolve_component_text(index, text, &a))
        },
        |t| resolve_component_text(index, t, area),
    )
}

pub fn text_to_component(index: &Indexer, text: &str, path: &Path) -> Option<M2Item> {
    let begining = text.split('/').next().unwrap_or("");

    if begining.chars().next().unwrap_or('a') == '.' {
        let mut path = path.to_path_buf();
        path.pop();
        Some(M2Item::RelComponent(text.into(), path))
    } else if text.split('/').count() > 1
        && begining.matches('_').count() == 1
        && begining.chars().next().unwrap_or('a').is_uppercase()
    {
        let mut parts = text.splitn(2, '/');
        let mod_name = parts.next()?.to_string();
        let mod_path = index.get_module_path(&mod_name)?;
        Some(M2Item::ModComponent(
            mod_name,
            parts.next()?.into(),
            mod_path,
        ))
    } else {
        Some(M2Item::Component(text.into()))
    }
}

fn update_index_from_config(index: &ArcIndexer, content: &str, area: &M2Area) {
    let tree = tree_sitter_parsers::parse(content, "javascript");
    let query = queries::js_require_config();

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(query, tree.root_node(), content.as_bytes());

    for m in matches {
        let key = get_node_text(m.captures[2].node, content);
        let val = get_node_text(m.captures[3].node, content);
        {
            let mut index = index.lock();
            match get_kind(m.captures[1].node, content) {
                Some(JSTypes::Map | JSTypes::Paths) => index.add_component_map(key, val, area),
                Some(JSTypes::Mixins) => index.add_component_mixin(key, val),
                None => continue,
            };
        }
    }
}

fn get_kind(node: Node, content: &str) -> Option<JSTypes> {
    match get_node_text(node, content) {
        "map" => Some(JSTypes::Map),
        "paths" => Some(JSTypes::Paths),
        "mixins" => Some(JSTypes::Mixins),
        _ => None,
    }
}

fn get_node_text<'a>(node: Node, content: &'a str) -> &'a str {
    let result = node
        .utf8_text(content.as_bytes())
        .unwrap_or("")
        .trim_matches('\\');

    if node.kind() == "string" {
        get_node_text(node.child(0).unwrap_or(node), content)
            .chars()
            .next()
            .map_or(result, |trim| result.trim_matches(trim))
    } else {
        result
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_update_index_from_config() {
        let index = Indexer::new();
        let content = r#"
        var config = {
            map: {
                '*': {
                    'some/js/component': 'Some_Model/js/component',
                    otherComp: 'Some_Other/js/comp'
                }
            },
            "paths": {
                'other/core/extension': 'Other_Module/js/core_ext',
                prototype: 'Something_Else/js/prototype.min'
            },
            config: {
                mixins: {
                    "Mage_Module/js/smth" : {
                        "My_Module/js/mixin/smth" : true
                    },
                    Adobe_Module: {
                        "My_Module/js/mixin/adobe": true
                    },
                }
            }
        };
        "#;

        let arc_index = index.into_arc();
        update_index_from_config(&arc_index, content, &M2Area::Base);

        let mut result = Indexer::new();
        result.add_component_map(
            "other/core/extension",
            "Other_Module/js/core_ext",
            &M2Area::Base,
        );
        result.add_component_map(
            "prototype",
            "Something_Else/js/prototype.min",
            &M2Area::Base,
        );
        result.add_component_map(
            "some/js/component",
            "Some_Model/js/component",
            &M2Area::Base,
        );
        result.add_component_map("otherComp", "Some_Other/js/comp", &M2Area::Base);
        result.add_component_mixin("Mage_Module/js/smth", "My_Module/js/mixin/smth");
        result.add_component_mixin("Adobe_Module", "My_Module/js/mixin/adobe");

        // FIX fix test without using to_owned
        // assert_eq!(arc_index.lock().to_owned(), result);
    }

    #[test]
    fn get_item_from_pos_mod_component() {
        let item = get_test_item(
            r#"
            define([
                'Some_Module/some/vie|w',
            ], function (someView) {})
            "#,
            "/a/b/c",
        );
        assert_eq!(
            item,
            Some(M2Item::ModComponent(
                "Some_Module".into(),
                "some/view".into(),
                PathBuf::from("/a/b/c/Some_Module")
            ))
        );
    }

    #[test]
    fn get_item_from_pos_component() {
        let item = get_test_item(
            r#"
            define([
                'jqu|ery',
            ], function ($) {})
            "#,
            "/a/b/c",
        );
        assert_eq!(item, Some(M2Item::Component("jquery".into())));
    }

    #[test]
    fn get_item_from_pos_component_with_slashes() {
        let item = get_test_item(
            r#"
            define([
                'jqu|ery-ui-modules/widget',
            ], function (widget) {})
            "#,
            "/a/b/c",
        );
        assert_eq!(
            item,
            Some(M2Item::Component("jquery-ui-modules/widget".into()))
        );
    }

    fn get_test_item(xml: &str, path: &str) -> Option<M2Item> {
        let win_path = format!("c:{}", path.replace('/', "\\"));
        let mut character = 0;
        let mut line = 0;
        for l in xml.lines() {
            if l.contains('|') {
                character = l.find('|').expect("Test has to have a | character") as u32;
                break;
            }
            line += 1;
        }
        let pos = Position { line, character };
        let uri = PathBuf::from(if cfg!(windows) { &win_path } else { path });
        let mut index = Indexer::new();
        index.add_module_path("Some_Module", PathBuf::from("/a/b/c/Some_Module"));
        get_item_from_pos(&index, &xml.replace('|', ""), &uri, pos)
    }
}
