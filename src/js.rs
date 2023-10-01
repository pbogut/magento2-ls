use std::path::{Path, PathBuf};

use glob::glob;
use lsp_types::Position;
use tree_sitter::{Node, Query, QueryCursor};

use crate::{
    indexer::{ArcIndexer, Indexer},
    m2::{M2Item, M2Path},
    ts::node_at_position,
};

enum JSTypes {
    Map,
    Paths,
    Mixins,
}

pub fn update_index(index: &ArcIndexer, path: &PathBuf) {
    let modules = glob(&path.append(&["**", "requirejs-config.js"]).to_path_string())
        .expect("Failed to read glob pattern");

    for require_config in modules {
        require_config.map_or_else(
            |_e| panic!("buhu"),
            |file_path| {
                let content = std::fs::read_to_string(file_path)
                    .expect("Should have been able to read the file");

                update_index_from_config(index, &content);
            },
        );
    }
}

pub fn get_item_from_position(index: &Indexer, path: &PathBuf, pos: Position) -> Option<M2Item> {
    let content = index.get_file(path)?;
    get_item_from_pos(index, content, path, pos)
}

fn get_item_from_pos(index: &Indexer, content: &str, path: &Path, pos: Position) -> Option<M2Item> {
    let query = r#"
    (
      (identifier) @def (#eq? @def define)
      (arguments (array (string) @str))
    )
    "#;
    let tree = tree_sitter_parsers::parse(content, "javascript");
    let query = Query::new(tree.language(), query)
        .map_err(|e| eprintln!("Error creating query: {:?}", e))
        .expect("Error creating query");

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    for m in matches {
        if node_at_position(m.captures[1].node, pos) {
            let text = get_node_text(m.captures[1].node, content);
            let text = resolve_component_text(index, &text)?;
            return text_to_component(index, text, path);
        }
    }

    None
}

pub fn resolve_component_text(index: &Indexer, text: &str) -> Option<String> {
    index.get_component_map(text).map_or_else(
        || Some(text.to_string()),
        |t| resolve_component_text(index, t),
    )
}

pub fn text_to_component(index: &Indexer, text: String, path: &Path) -> Option<M2Item> {
    let begining = text.split('/').next().unwrap_or("");

    if begining.chars().next().unwrap_or('a') == '.' {
        let mut path = path.to_path_buf();
        path.pop();
        Some(M2Item::RelComponent(text, path))
    } else if text.split('/').count() > 1
        && begining.matches('_').count() == 1
        && begining.chars().next().unwrap_or('a').is_uppercase()
    {
        let mut parts = text.splitn(2, '/');
        let mod_name = parts.next()?.to_string();
        let mod_path = index.get_module_path(&mod_name)?;
        Some(M2Item::ModComponent(
            mod_name,
            parts.next()?.to_string(),
            mod_path,
        ))
    } else {
        Some(M2Item::Component(text))
    }
}

fn update_index_from_config(index: &ArcIndexer, content: &str) {
    let map_query = r#"
    (
      (identifier) @config
      (object (pair [(property_identifier) (string)] @mapkey
          (object (pair (object (pair
            [(property_identifier) (string)] @key + (string) @val
          ))))
      ))

      (#eq? @config config)
      (#match? @mapkey "[\"']?map[\"']?")
    )
    "#;

    let mixins_query = r#"
    (
      (identifier) @config
      (object (pair [(property_identifier) (string)] ; @configkey
        (object (pair [(property_identifier) (string)] @mixins
          (object (pair [(property_identifier) (string)] @key
            (object (pair [(property_identifier) (string)] @val (true)))
          ))
        ))
      ))

      (#match? @config config)
      ; (#match? @configkey "[\"']?config[\"']?")
      (#match? @mixins "[\"']?mixins[\"']?")
    )
    "#;

    let path_query = r#"
    (
      (identifier) @config
      (object (pair [(property_identifier) (string)] @pathskey
        (((object (pair
          [(property_identifier) (string)] @key + (string) @val
        ))))
      ))

      (#eq? @config config)
      (#match? @pathskey "[\"']?paths[\"']?")
    )
    "#;

    let query = format!("{} {} {}", map_query, path_query, mixins_query);
    let tree = tree_sitter_parsers::parse(content, "javascript");
    let query = Query::new(tree.language(), &query)
        .map_err(|e| eprintln!("Error creating query: {:?}", e))
        .expect("Error creating query");

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    for m in matches {
        let key = get_node_text(m.captures[2].node, content);
        let val = get_node_text(m.captures[3].node, content);
        {
            let mut index = index.lock();
            match get_kind(m.captures[1].node, content) {
                Some(JSTypes::Map | JSTypes::Paths) => index.add_component_map(&key, val),
                Some(JSTypes::Mixins) => index.add_component_mixin(&key, val),
                None => continue,
            };
        }
    }
}

fn get_kind(node: Node, content: &str) -> Option<JSTypes> {
    match get_node_text(node, content).as_str() {
        "map" => Some(JSTypes::Map),
        "paths" => Some(JSTypes::Paths),
        "mixins" => Some(JSTypes::Mixins),
        _ => None,
    }
}

fn get_node_text(node: Node, content: &str) -> String {
    let result = node
        .utf8_text(content.as_bytes())
        .unwrap_or("")
        .trim_matches('\\')
        .to_string();

    if node.kind() == "string" {
        match get_node_text(node.child(0).unwrap_or(node), content)
            .chars()
            .next()
        {
            Some(trim) => result.trim_matches(trim).to_string(),
            None => result,
        }
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
        update_index_from_config(&arc_index, content);

        let mut result = Indexer::new();
        result.add_component_map(
            "other/core/extension",
            "Other_Module/js/core_ext".to_string(),
        );
        result.add_component_map("prototype", "Something_Else/js/prototype.min");
        result.add_component_map("some/js/component", "Some_Model/js/component");
        result.add_component_map("otherComp", "Some_Other/js/comp");
        result.add_component_mixin("Mage_Module/js/smth", "My_Module/js/mixin/smth");
        result.add_component_mixin("Adobe_Module", "My_Module/js/mixin/adobe");

        assert_eq!(arc_index.lock().to_owned(), result);
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
                "Some_Module".to_string(),
                "some/view".to_string(),
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
        assert_eq!(item, Some(M2Item::Component("jquery".to_string())));
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
            Some(M2Item::Component("jquery-ui-modules/widget".to_string()))
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
