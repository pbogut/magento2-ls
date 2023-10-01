use lsp_types::Position;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tree_sitter::{Node, Query, QueryCursor};

use crate::{
    indexer::Indexer,
    js,
    m2_types::{M2Area, M2Item, M2Path},
    ts::{get_node_text, get_node_text_before_pos, node_at_position},
};

#[derive(Debug, Clone)]
enum XmlPart {
    Text,
    Attribute(String),
    None,
}

#[derive(Debug, Clone)]
pub enum PathDepth {
    #[allow(dead_code)]
    Any,
    Attribute,
    #[allow(dead_code)]
    Tags(usize),
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlCompletion {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone)]
struct XmlTag {
    name: String,
    attributes: HashMap<String, String>,
    text: String,
    hover_on: XmlPart,
}

impl XmlTag {
    fn new() -> Self {
        Self {
            name: String::new(),
            attributes: HashMap::new(),
            text: String::new(),
            hover_on: XmlPart::None,
        }
    }
}

pub fn get_current_position_path(
    content: &str,
    pos: Position,
    depth: &PathDepth,
) -> Option<XmlCompletion> {
    let query_string = "
        (tag_name) @tag_name
        (attribute_value) @attr_val
    ";

    let tree = tree_sitter_parsers::parse(content, "html");
    let query = Query::new(tree.language(), query_string)
        .map_err(|e| eprintln!("Error creating query: {:?}", e))
        .expect("Error creating query");
    let mut cursor = QueryCursor::new();
    let captures = cursor.captures(&query, tree.root_node(), content.as_bytes());
    for (m, _) in captures {
        let node = m.captures[0].node;
        if node_at_position(node, pos) {
            let path = node_to_path(node, content, depth)?;
            let text = get_node_text_before_pos(node, content, pos);
            return Some(XmlCompletion { path, text });
        }
    }
    None
}

fn node_walk_back(node: Node) -> Option<Node> {
    node.prev_sibling().map_or_else(|| node.parent(), Some)
}

fn node_to_path(node: Node, content: &str, depth: &PathDepth) -> Option<String> {
    let mut path = vec![];
    let mut current_node = node;
    let mut has_attr = false;
    let mut tags_count = 0;
    let mut node_ids = vec![];
    while let Some(node) = node_walk_back(current_node) {
        current_node = node;
        if node_ids.contains(&node.id()) {
            continue;
        }
        node_ids.push(node.id());
        if node.kind() == "attribute_name" && !has_attr {
            has_attr = true;
            let attr_name = get_node_text(node, content);
            path.push((node.kind(), attr_name));
        } else if node.kind() == "self_closing_tag" || node.kind() == "start_tag" {
            if node.child(0).is_some() {
                if node_ids.contains(&node.child(0)?.id()) {
                    continue;
                }
                path.push((node.kind(), get_node_text(node.child(1)?, content)));
                tags_count += 1;
            }
        } else if node.kind() == "tag_name" {
            path.push((node.kind(), get_node_text(node, content)));
            tags_count += 1;
        }

        match depth {
            PathDepth::Any => (),
            PathDepth::Attribute => {
                if has_attr {
                    break;
                }
            }
            PathDepth::Tags(level) => {
                if tags_count == *level {
                    break;
                }
            }
        }
    }
    path.reverse();
    let mut result = String::new();
    for (kind, name) in path {
        match kind {
            "attribute_name" => result.push_str(&format!("[@{}]", name)),
            "self_closing_tag" | "start_tag" | "tag_name" => {
                result.push_str(&format!("/{}", &name));
            }

            _ => (),
        }
    }
    Some(result)
}

pub fn get_item_from_position(index: &Indexer, path: &PathBuf, pos: Position) -> Option<M2Item> {
    let content = index.get_file(path)?;
    get_item_from_pos(index, content, path, pos)
}

fn get_item_from_pos(
    index: &Indexer,
    content: &str,
    path: &PathBuf,
    pos: Position,
) -> Option<M2Item> {
    let tag = get_xml_tag_at_pos(content, pos)?;

    match tag.hover_on {
        XmlPart::Attribute(ref attr_name) => match attr_name.as_str() {
            "method" | "instance" | "class" => try_method_item_from_tag(&tag).or_else(|| {
                try_any_item_from_str(tag.attributes.get(attr_name)?, &path.get_area())
            }),
            "template" => try_phtml_item_from_str(tag.attributes.get(attr_name)?, &path.get_area()),
            _ => try_any_item_from_str(tag.attributes.get(attr_name)?, &path.get_area()),
        },
        XmlPart::Text => {
            let text = tag.text.trim_matches('\\');
            let empty = String::new();
            let xsi_type = tag.attributes.get("xsi:type").unwrap_or(&empty);

            match xsi_type.as_str() {
                "object" => Some(get_class_item_from_str(text)),
                "init_parameter" => try_const_item_from_str(text),
                "string" => {
                    if tag.attributes.get("name") == Some(&"component".to_string()) {
                        let text = js::resolve_component_text(index, text)?;
                        js::text_to_component(index, text, path)
                    } else {
                        try_any_item_from_str(text, &path.get_area())
                    }
                }
                _ => try_any_item_from_str(text, &path.get_area()),
            }
        }
        XmlPart::None => None,
    }
}

fn get_xml_tag_at_pos(content: &str, pos: Position) -> Option<XmlTag> {
    let query_string = "
    (element
        (start_tag
            (tag_name) @tag_name
            (attribute
                (attribute_name) @attr_name
                (quoted_attribute_value (attribute_value) @attr_val)
            )?
        ) @tag
        (text)? @text
    )
    (element
        (self_closing_tag
            (tag_name) @tag_name
            (attribute
                (attribute_name) @attr_name
                (quoted_attribute_value (attribute_value) @attr_val)
            )
        ) @tag
    )
    ";

    let tree = tree_sitter_parsers::parse(content, "html");
    let query = Query::new(tree.language(), query_string)
        .map_err(|e| eprintln!("Error creating query: {:?}", e))
        .expect("Error creating query");

    let mut cursor = QueryCursor::new();
    let captures = cursor.captures(&query, tree.root_node(), content.as_bytes());

    let mut last_attribute_name = String::new();
    let mut last_tag_id: Option<usize> = None;
    let mut tag = XmlTag::new();

    for (m, i) in captures {
        let first = m.captures[0].node; // always (self)opening tag
        let last = m.captures[m.captures.len() - 1].node;
        if !node_at_position(first, pos) && !node_at_position(last, pos) {
            continue;
        }
        let id = m.captures[0].node.id(); // id of tag name
        if last_tag_id.is_none() || last_tag_id != Some(id) {
            last_tag_id = Some(id);
            tag = XmlTag::new();
        }
        let node = m.captures[i].node;
        let hovered = node_at_position(node, pos);
        match node.kind() {
            "tag_name" => {
                tag.name = get_node_text(node, content);
            }
            "attribute_name" => {
                last_attribute_name = get_node_text(node, content);
            }
            "attribute_value" => {
                tag.attributes
                    .insert(last_attribute_name.clone(), get_node_text(node, content));
                if hovered {
                    tag.hover_on = XmlPart::Attribute(last_attribute_name.clone());
                }
            }
            "text" => {
                tag.text = get_node_text(node, content);
                if hovered {
                    tag.hover_on = XmlPart::Text;
                }
            }
            _ => (),
        }
    }

    match tag.hover_on {
        XmlPart::None => None,
        _ => Some(tag),
    }
}

fn try_any_item_from_str(text: &str, area: &M2Area) -> Option<M2Item> {
    if does_ext_eq(text, "phtml") {
        try_phtml_item_from_str(text, area)
    } else if text.contains("::") {
        try_const_item_from_str(text)
    } else if text.chars().next()?.is_uppercase() {
        Some(get_class_item_from_str(text))
    } else {
        None
    }
}

fn try_const_item_from_str(text: &str) -> Option<M2Item> {
    if text.split("::").count() == 2 {
        let mut parts = text.split("::");
        Some(M2Item::Const(
            parts.next()?.to_string(),
            parts.next()?.to_string(),
        ))
    } else {
        None
    }
}

fn get_class_item_from_str(text: &str) -> M2Item {
    M2Item::Class(text.to_string())
}

fn try_phtml_item_from_str(text: &str, area: &M2Area) -> Option<M2Item> {
    if text.split("::").count() == 2 {
        let mut parts = text.split("::");
        match area {
            M2Area::Frontend => Some(M2Item::FrontPhtml(
                parts.next()?.to_string(),
                parts.next()?.to_string(),
            )),
            M2Area::Adminhtml => Some(M2Item::AdminPhtml(
                parts.next()?.to_string(),
                parts.next()?.to_string(),
            )),
            M2Area::Base => Some(M2Item::BasePhtml(
                parts.next()?.to_string(),
                parts.next()?.to_string(),
            )),
            M2Area::Unknown => None,
        }
    } else {
        None
    }
}

fn try_method_item_from_tag(tag: &XmlTag) -> Option<M2Item> {
    if tag.attributes.get("instance").is_some() && tag.attributes.get("method").is_some() {
        Some(M2Item::Method(
            tag.attributes.get("instance")?.to_string(),
            tag.attributes.get("method")?.to_string(),
        ))
    } else if tag.attributes.get("class").is_some() && tag.attributes.get("method").is_some() {
        Some(M2Item::Method(
            tag.attributes.get("class")?.to_string(),
            tag.attributes.get("method")?.to_string(),
        ))
    } else {
        None
    }
}

fn does_ext_eq(path: &str, ext: &str) -> bool {
    Path::new(path)
        .extension()
        .map_or(false, |e| e.eq_ignore_ascii_case(ext))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::PathBuf;

    fn get_position_from_test_xml(xml: &str) -> Position {
        let mut character = 0;
        let mut line = 0;
        for l in xml.lines() {
            if l.contains('|') {
                character = l.find('|').expect("Test has to have a | character") as u32;
                break;
            }
            line += 1;
        }
        Position { line, character }
    }

    fn get_test_position_path(xml: &str, depth: &PathDepth) -> Option<XmlCompletion> {
        let pos = get_position_from_test_xml(xml);
        get_current_position_path(&xml.replace('|', ""), pos, depth)
    }

    fn get_test_item_from_pos(xml: &str, path: &str) -> Option<M2Item> {
        let win_path = format!("c:{}", path.replace('/', "\\"));
        let pos = get_position_from_test_xml(xml);
        let uri = PathBuf::from(if cfg!(windows) { &win_path } else { path });
        let index = Indexer::new();
        get_item_from_pos(&index, &xml.replace('|', ""), &uri, pos)
    }

    #[test]
    fn test_get_item_from_pos_class_in_tag_text() {
        let item = get_test_item_from_pos(r#"<?xml version="1.0"?><item>|A\B\C</item>"#, "/a/b/c");

        assert_eq!(item, Some(M2Item::Class("A\\B\\C".to_string())));
    }

    #[test]
    fn test_get_item_from_pos_template_in_tag_attribute() {
        let item = get_test_item_from_pos(
            r#"<?xml version="1.0"?><block template="Some_|Module::path/to/file.phtml"></block>"#,
            "/a/design/adminhtml/c",
        );
        assert_eq!(
            item,
            Some(M2Item::AdminPhtml(
                "Some_Module".to_string(),
                "path/to/file.phtml".to_string()
            ))
        );
    }

    #[test]
    fn test_get_item_from_pos_frontend_template_in_tag_attribute() {
        let item = get_test_item_from_pos(
            r#"<?xml version="1.0"?><block template="Some_Module::path/t|o/file.phtml"></block>"#,
            "/a/view/frontend/c",
        );
        assert_eq!(
            item,
            Some(M2Item::FrontPhtml(
                "Some_Module".to_string(),
                "path/to/file.phtml".to_string()
            ))
        );
    }

    #[test]
    fn test_get_item_from_pos_method_in_job_tag_attribute() {
        let item = get_test_item_from_pos(
            r#"<?xml version="1.0"?><job instance="\A\B\C\" method="met|Hod"></job>"#,
            "/a/a/c",
        );
        assert_eq!(
            item,
            Some(M2Item::Method("A\\B\\C".to_string(), "metHod".to_string()))
        );
    }

    #[test]
    fn test_get_item_from_pos_method_in_service_tag_attribute() {
        let item = get_test_item_from_pos(
            r#"<?xml version="1.0"?><service class="A\B\C\" method="met|Hod"></service>"#,
            "/a/a/c",
        );
        assert_eq!(
            item,
            Some(M2Item::Method("A\\B\\C".to_string(), "metHod".to_string()))
        );
    }

    #[test]
    fn test_get_item_from_pos_class_in_service_tag_attribute() {
        let item = get_test_item_from_pos(
            r#"<?xml version="1.0"?><service class="\|A\B\C" method="metHod">xx</service>"#,
            "/a/a/c",
        );
        assert_eq!(
            item,
            Some(M2Item::Method("A\\B\\C".to_string(), "metHod".to_string()))
        );
    }

    #[test]
    fn test_get_item_from_pos_attribute_in_tag_with_method() {
        let item = get_test_item_from_pos(
            r#"<?xml version="1.0"?><service something="\|A\B\C" method="metHod">xx</service>"#,
            "/a/a/c",
        );
        assert_eq!(item, Some(M2Item::Class("A\\B\\C".to_string())));
    }

    #[test]
    fn test_get_item_from_pos_class_in_text_in_tag() {
        let item = get_test_item_from_pos(r#"<?xml version="1.0"?><some>|A\B\C</some>"#, "/a/a/c");
        assert_eq!(item, Some(M2Item::Class("A\\B\\C".to_string())));
    }

    #[test]
    fn test_get_item_from_pos_const_in_text_in_tag() {
        let item = get_test_item_from_pos(
            r#"<?xml version="1.0"?><some>\|A\B\C::CONST_ANT</some>"#,
            "/a/a/c",
        );
        assert_eq!(
            item,
            Some(M2Item::Const(
                "A\\B\\C".to_string(),
                "CONST_ANT".to_string()
            ))
        );
    }

    #[test]
    fn test_get_item_from_pos_template_in_text_in_tag() {
        let item = get_test_item_from_pos(
            r#"<?xml version="1.0"?><some>Some_Module::fi|le.phtml</some>"#,
            "/a/view/adminhtml/c",
        );
        assert_eq!(
            item,
            Some(M2Item::AdminPhtml(
                "Some_Module".to_string(),
                "file.phtml".to_string()
            ))
        );
    }

    #[test]
    fn test_get_item_from_pos_method_attribute_in_tag() {
        let item = get_test_item_from_pos(
            r#"<?xml version="1.0"?><service something="\A\B\C" method="met|Hod">xx</service>"#,
            "/a/a/c",
        );
        assert_eq!(item, None)
    }

    #[test]
    fn test_should_get_most_inner_tag_from_nested() {
        let item = get_test_item_from_pos(
            r#"<?xml version=\"1.0\"?>
                <type name="Magento\Elasticsearch\Model\Adapter\BatchDataMapper\ProductDataMapper">
                    <arguments>
                        <argument template="Some_Module::template.phtml" xsi:type="object">
                            <item name="boolean" xsi:type="object">Some\Cl|ass\Name</item>
                            <item name="multiselect" xsi:type="string">multiselect</item>
                            <item name="select" xsi:type="string">select</item>
                            \\A\\B\\C
                        </argument>
                    </arguments>
                </type>
            "#,
            "/a/a/c",
        );
        assert_eq!(item, Some(M2Item::Class("Some\\Class\\Name".to_string())))
    }

    #[test]
    fn test_should_get_class_from_class_attribute_of_block_tag() {
        let item = get_test_item_from_pos(
            r#"<?xml version=\"1.0\"?>
               <block class="A\|B\C" name="some_name" template="Some_Module::temp/file.phtml"/>
            "#,
            "/a/a/c",
        );
        assert_eq!(item, Some(M2Item::Class("A\\B\\C".to_string())))
    }

    #[test]
    fn test_get_current_position_path_when_starting_attribute() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="urn:magento:framework:ObjectManager/etc/config.xsd">
                <ala/>
                <type name="Klaviyo\Reclaim\Observer\SaveOrderMarketingConsent">
                    <plugin name="pharmacy_klaviyo_set_consent_and_subscribe"
                        template="Mo|du
            "#,
            &PathDepth::Any,
        );
        assert_eq!(
            item,
            Some(XmlCompletion {
                path: "/config/type/plugin[@template]".to_string(),
                text: "Mo".to_string()
            })
        )
    }

    #[test]
    fn test_get_current_position_path_when_starting_attribute_inside_tag() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block template="Modu|le
                    <plugin name="a_b_c"
                      type="A\B\C"/>
                </type>
            </config>
            "#,
            &PathDepth::Any,
        );
        assert_eq!(
            item,
            Some(XmlCompletion {
                path: "/config/type/block[@template]".into(),
                text: "Modu".into()
            })
        )
    }
}
