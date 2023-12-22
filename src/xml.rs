use lsp_types::{Position, Range};
use std::{collections::HashMap, path::PathBuf};
use tree_sitter::{Node, QueryCursor};

use crate::{
    js,
    m2::{self, M2Item, M2Path},
    queries,
    state::State,
    ts::{get_node_str, get_node_text_before_pos, node_at_position, node_last_child},
};

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum XmlPart {
    Text,
    Attribute(String),
    None,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlCompletion {
    pub path: String,
    pub text: String,
    pub range: Range,
    pub tag: Option<XmlTag>,
}

impl XmlCompletion {
    pub fn match_path(&self, text: &str) -> bool {
        self.path.ends_with(text)
    }

    pub fn attribute_eq(&self, attr: &str, val: &str) -> bool {
        self.tag.as_ref().map_or(false, |t| {
            t.attributes.get(attr).map_or(false, |v| v == val)
        })
    }

    pub fn attribute_in(&self, attr: &str, vals: &[&str]) -> bool {
        self.tag.as_ref().map_or(false, |t| {
            t.attributes
                .get(attr)
                .map_or(false, |v| vals.contains(&v.as_ref()))
        })
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlTag {
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

pub fn get_current_position_path(content: &str, pos: Position) -> Option<XmlCompletion> {
    let tree = tree_sitter_parsers::parse(content, "html");
    let query = queries::xml_current_position_path();
    let mut cursor = QueryCursor::new();
    let captures = cursor.captures(query, tree.root_node(), content.as_bytes());
    for (m, i) in captures {
        let node = m.captures[i].node;
        if node_at_position(node, pos) {
            let mut text = get_node_text_before_pos(node, content, pos);
            if node.kind() == ">" && text.is_empty() {
                // this is end of tag node but if text is empty
                // the tag is not really closed yet, just should be
                continue;
            }
            let mut start_col = node.start_position().column as u32;
            if node.kind() == "quoted_attribute_value" {
                if text == "\"" {
                    start_col += 1;
                    text = String::new();
                } else {
                    continue;
                }
            }
            if node.kind() == ">" && text == ">" {
                start_col += 1;
                text = String::new();
            }
            let path = node_to_path(node, content)?;
            let tag = node_to_tag(node, content);
            let range = Range {
                start: Position {
                    line: node.start_position().row as u32,
                    character: start_col,
                },
                end: pos,
            };
            return Some(XmlCompletion {
                path,
                text,
                range,
                tag,
            });
        }
    }
    None
}

// fn node_dive_in<'a>(node: Option<Node<'a>>, list: &mut Vec<Node<'a>>) {
//     if node.is_none() {
//         return;
//     }
//     if let Some(n) = node {
//         list.push(n.clone());
//         node_dive_in(n.child(0), list);
//         node_dive_in(n.next_sibling(), list);
//     }
// }

// fn node_walk_forward(node: Node) -> Vec<Node> {
//     let mut list = vec![];
//     node_dive_in(Some(node), &mut list);
//     list
// }

fn node_walk_back(node: Node) -> Option<Node> {
    node.prev_sibling().map_or_else(|| node.parent(), Some)
}

fn node_to_tag(node: Node, content: &str) -> Option<XmlTag> {
    let mut current_node = node;
    while let Some(node) = node_walk_back(current_node) {
        current_node = node;
        if node.kind() == "self_closing_tag" || node.kind() == "start_tag" {
            let text = get_node_str(node, content);
            if text.chars().last()? != '>' {
                return None;
            }
            return get_xml_tag_at_pos(
                text,
                Position {
                    line: 0,
                    character: 0,
                },
            );
        }
    }
    None
}

fn node_to_path(node: Node, content: &str) -> Option<String> {
    let mut path = vec![];
    let mut current_node = node;
    let mut has_attr = false;
    let mut node_ids = vec![];
    let mut on_text_node = false;
    let mut pop_last = false;
    let text = get_node_str(node, content);
    if node.kind() == ">" && text == ">" {
        on_text_node = true;
    }

    if node.kind() == "text" && node.prev_sibling().is_some() {
        if let Some(last) = node_last_child(node.prev_sibling()?) {
            if last.kind() == ">" && get_node_str(last, content) == ">" {
                on_text_node = true;
            }
        }
    }

    while let Some(node) = node_walk_back(current_node) {
        current_node = node;
        if node_ids.contains(&node.id()) {
            continue;
        }
        node_ids.push(node.id());
        if node.kind() == "attribute_name" && !has_attr {
            let attr_name = get_node_str(node, content);
            has_attr = true;
            path.push((node.kind(), attr_name));
        } else if node.kind() == "self_closing_tag" || node.kind() == "start_tag" {
            if node.child(0).is_some() {
                if node_ids.contains(&node.child(0)?.id()) {
                    continue;
                }
                path.push((node.kind(), get_node_str(node.child(1)?, content)));
            }
        } else if node.kind() == "tag_name" && node.parent()?.kind() != "end_tag" {
            path.push((node.kind(), get_node_str(node, content)));
        } else if node.kind() == "tag_name" && node.parent()?.kind() == "end_tag" {
            pop_last = true;
            on_text_node = false;
        }
    }
    path.reverse();
    if pop_last {
        path.pop();
    }
    if on_text_node {
        path.push(("text", "[$text]"));
    }
    let mut result = String::new();
    for (kind, name) in path {
        match kind {
            "text" => result.push_str(name),
            "attribute_name" => {
                result.push_str("[@");
                result.push_str(name);
                result.push(']');
            }
            "self_closing_tag" | "start_tag" | "tag_name" => {
                result.push_str(&format!("/{}", name));
            }

            _ => (),
        }
    }
    Some(result)
}

pub fn get_item_from_position(state: &State, path: &PathBuf, pos: Position) -> Option<M2Item> {
    let content = state.get_file(path)?;
    get_item_from_pos(state, content, path, pos)
}

fn get_item_from_pos(
    state: &State,
    content: &str,
    path: &PathBuf,
    pos: Position,
) -> Option<M2Item> {
    let tag = get_xml_tag_at_pos(content, pos)?;

    match tag.hover_on {
        XmlPart::Attribute(ref attr_name) => match attr_name.as_str() {
            "method" | "instance" | "class" => try_method_item_from_tag(&tag).or_else(|| {
                m2::try_any_item_from_str(tag.attributes.get(attr_name)?, &path.get_area())
            }),
            "template" => {
                m2::try_phtml_item_from_str(tag.attributes.get(attr_name)?, &path.get_area())
            }
            _ => m2::try_any_item_from_str(tag.attributes.get(attr_name)?, &path.get_area()),
        },
        XmlPart::Text => {
            let text = tag.text.trim_matches('\\');
            let empty = String::new();
            let xsi_type = tag.attributes.get("xsi:type").unwrap_or(&empty);

            match xsi_type.as_str() {
                "object" => Some(m2::get_class_item_from_str(text)),
                "init_parameter" => m2::try_const_item_from_str(text),
                "string" => {
                    if tag.attributes.get("name").is_some_and(|s| s == "component") {
                        js::text_to_component(state, text, path)
                    } else {
                        m2::try_any_item_from_str(text, &path.get_area())
                    }
                }
                _ => m2::try_any_item_from_str(text, &path.get_area()),
            }
        }
        XmlPart::None => None,
    }
}

fn get_xml_tag_at_pos(content: &str, pos: Position) -> Option<XmlTag> {
    let tree = tree_sitter_parsers::parse(content, "html");
    let query = queries::xml_tag_at_pos();

    let mut cursor = QueryCursor::new();
    let captures = cursor.captures(query, tree.root_node(), content.as_bytes());

    let mut last_attribute_name = "";
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
                tag.name = get_node_str(node, content).into();
            }
            "attribute_name" => {
                last_attribute_name = get_node_str(node, content);
                tag.attributes
                    .insert(last_attribute_name.into(), String::new());
            }
            "attribute_value" => {
                tag.attributes.insert(
                    last_attribute_name.into(),
                    get_node_str(node, content).into(),
                );
                if hovered {
                    tag.hover_on = XmlPart::Attribute(last_attribute_name.into());
                }
            }
            "text" => {
                tag.text = get_node_str(node, content).into();
                if hovered {
                    tag.hover_on = XmlPart::Text;
                }
            }
            _ => (),
        }
    }

    if tag.name.is_empty() {
        return None;
    }

    Some(tag)
}

fn try_method_item_from_tag(tag: &XmlTag) -> Option<M2Item> {
    if tag.attributes.get("instance").is_some() && tag.attributes.get("method").is_some() {
        Some(M2Item::Method(
            tag.attributes.get("instance")?.into(),
            tag.attributes.get("method")?.into(),
        ))
    } else if tag.attributes.get("class").is_some() && tag.attributes.get("method").is_some() {
        Some(M2Item::Method(
            tag.attributes.get("class")?.into(),
            tag.attributes.get("method")?.into(),
        ))
    } else {
        None
    }
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

    fn get_test_position_path(xml: &str) -> Option<XmlCompletion> {
        let pos = get_position_from_test_xml(xml);
        get_current_position_path(&xml.replace('|', ""), pos)
    }

    fn get_test_item_from_pos(xml: &str, path: &str) -> Option<M2Item> {
        let win_path = format!("c:{}", path.replace('/', "\\"));
        let pos = get_position_from_test_xml(xml);
        let uri = PathBuf::from(if cfg!(windows) { &win_path } else { path });
        let state = State::new();
        get_item_from_pos(&state, &xml.replace('|', ""), &uri, pos)
    }

    fn get_test_xml_tag_at_pos(xml: &str) -> Option<XmlTag> {
        let pos = get_position_from_test_xml(xml);
        get_xml_tag_at_pos(&xml.replace('|', ""), pos)
    }

    #[test]
    fn test_get_item_from_pos_class_in_tag_text() {
        let item = get_test_item_from_pos(r#"<?xml version="1.0"?><item>|A\B\C</item>"#, "/a/b/c");

        assert_eq!(item, Some(M2Item::Class("A\\B\\C".into())));
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
                "Some_Module".into(),
                "path/to/file.phtml".into()
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
                "Some_Module".into(),
                "path/to/file.phtml".into()
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
            Some(M2Item::Method("A\\B\\C".into(), "metHod".into()))
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
            Some(M2Item::Method("A\\B\\C".into(), "metHod".into()))
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
            Some(M2Item::Method("A\\B\\C".into(), "metHod".into()))
        );
    }

    #[test]
    fn test_get_item_from_pos_attribute_in_tag_with_method() {
        let item = get_test_item_from_pos(
            r#"<?xml version="1.0"?><service something="\|A\B\C" method="metHod">xx</service>"#,
            "/a/a/c",
        );
        assert_eq!(item, Some(M2Item::Class("A\\B\\C".into())));
    }

    #[test]
    fn test_get_item_from_pos_class_in_text_in_tag() {
        let item = get_test_item_from_pos(r#"<?xml version="1.0"?><some>|A\B\C</some>"#, "/a/a/c");
        assert_eq!(item, Some(M2Item::Class("A\\B\\C".into())));
    }

    #[test]
    fn test_get_item_from_pos_const_in_text_in_tag() {
        let item = get_test_item_from_pos(
            r#"<?xml version="1.0"?><some>\|A\B\C::CONST_ANT</some>"#,
            "/a/a/c",
        );
        assert_eq!(
            item,
            Some(M2Item::Const("A\\B\\C".into(), "CONST_ANT".into()))
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
                "Some_Module".into(),
                "file.phtml".into()
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
        assert_eq!(item, Some(M2Item::Class("Some\\Class\\Name".into())))
    }

    #[test]
    fn test_should_get_class_from_class_attribute_of_block_tag() {
        let item = get_test_item_from_pos(
            r#"<?xml version=\"1.0\"?>
               <block class="A\|B\C" name="some_name" template="Some_Module::temp/file.phtml"/>
            "#,
            "/a/a/c",
        );
        assert_eq!(item, Some(M2Item::Class("A\\B\\C".into())))
    }

    #[test]
    fn test_get_current_position_path_when_starting_inside_attribute() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="urn:magento:framework:ObjectManager/etc/config.xsd">
                <ala/>
                <type name="Klaviyo\Reclaim\Observer\SaveOrderMarketingConsent">
                    <plugin name="pharmacy_klaviyo_set_consent_and_subscribe"
                        template="Mo|du
            "#,
        );
        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/plugin[@template]");
        assert_eq!(item.text, "Mo");
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
        );

        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/block[@template]");
        assert_eq!(item.text, "Modu");
    }

    #[test]
    fn test_get_current_position_path_when_in_empty_attribute_value() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block class="|"
                    <plugin name="a_b_c"
                      type="A\B\C"/>
                </type>
            </config>
            "#,
        );

        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/block[@class]");
        assert_eq!(item.text, "");
    }

    #[test]
    fn test_get_current_position_path_when_after_empty_attribute_value() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block class=""|
                    <plugin name="a_b_c"
                      type="A\B\C"/>
                </type>
            </config>
            "#,
        );

        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/block");
        assert_eq!(item.text, "");
        assert!(item.tag.is_none());
    }

    #[test]
    fn test_get_current_position_path_when_before_empty_attribute_value() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block class=|""
                    <plugin name="a_b_c"
                      type="A\B\C"/>
                </type>
            </config>
            "#,
        );

        assert!(item.is_none()); // nothig to complete here
    }

    #[test]
    fn test_get_current_position_path_when_starting_inside_tag() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block>|Nana
                    <plugin name="a_b_c"
                      type="A\B\C"/>
                </type>
            </config>
            "#,
        );
        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/block[$text]");
        assert_eq!(item.text, "");
        assert!(item.tag.is_none());
    }

    #[test]
    fn test_get_current_position_path_when_inside_tag() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block>Nan|a
                    <plugin name="a_b_c"
                      type="A\B\C"/>
                </type>
            </config>
            "#,
        );

        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/block[$text]");
        assert_eq!(item.text, "Nan");
        assert!(item.tag.is_none());
    }

    #[test]
    fn test_get_current_position_path_outside_attribute_and_text() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="urn:magento:framework:Event/etc/events.xsd">
                <item xsi:type="object"|
                <item/>
            </config>
            "#,
        );

        let item = item.unwrap();
        assert_eq!(item.path, "/config/item");
        assert_eq!(item.text, "");
        assert!(item.tag.is_none());
    }

    #[test]
    fn test_get_current_position_path_between_start_and_end_tag() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <page>
                <body>
                    <referenceBlock name="checkout.root">
                        <arguments>
                            <argument name="jsLayout" xsi:type="array">
                                <item name="component" xsi:type="string">|</item>
                            </argument>
                        </arguments>
                    </referenceBlock>
                </body>
            </page>
            "#,
        );

        let item = dbg!(item).unwrap();
        assert!(item.attribute_eq("xsi:type", "string"));
        assert!(item.attribute_eq("name", "component"));
    }

    #[test]
    fn test_get_xml_tag_at_position_0_when_content_is_opening_tag() {
        let item = get_test_xml_tag_at_pos(r#"|<item attribute="value" name="other">"#);

        let item = item.unwrap();
        assert_eq!(item.name, "item");
        assert!(item.attributes.get("name").is_some());
        assert!(item.attributes.get("attribute").is_some());
    }

    #[test]
    fn test_unfinished_xml_at_text_not_empty() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block>Nan|a
            "#,
        );

        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/block[$text]");
        assert_eq!(item.text, "Nan");
        assert!(item.tag.is_none());
    }

    #[test]
    fn test_unfinished_xml_at_text_empty() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block>|
            "#,
        );

        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/block[$text]");
        assert_eq!(item.text, "");
        assert!(item.tag.is_none());
    }

    #[test]
    fn test_unfinished_xml_tag_not_closed() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block|
            "#,
        );

        let item = item.unwrap();
        assert!(!item.match_path("[$text]"));
    }

    #[test]
    fn test_unfinished_current_tag_at_text_not_empty() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block>Nan|a
                </type>
            </config>
            "#,
        );

        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/block[$text]");
        assert_eq!(item.text, "Nan");
        assert!(item.tag.is_none());
    }

    #[test]
    fn test_unfinished_current_tag_at_text_empty() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block>|
                </type>
            </config>
            "#,
        );

        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/block[$text]");
        assert_eq!(item.text, "");
        assert!(item.tag.is_none());
    }

    #[test]
    fn test_unfinished_current_tag_tag_not_closed() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block|
                </type>
            </config>
            "#,
        );

        let item = item.unwrap();
        assert!(!item.match_path("[$text]"));
    }

    #[test]
    fn test_valid_xml_at_text_not_empty() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block>Nan|a</blocK>
                </type>
            </config>
            "#,
        );

        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/block[$text]");
        assert_eq!(item.text, "Nan");
        assert!(item.tag.is_none());
    }

    #[test]
    fn test_valid_xml_at_text_empty() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block>|</block>
                </type>
            </config>
            "#,
        );

        let item = item.unwrap();
        assert_eq!(item.path, "/config/type/block[$text]");
        assert_eq!(item.text, "");
        assert!(item.tag.is_none());
    }

    #[test]
    fn test_valid_xml_tag_not_closed() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block|</block>
                </type>
            </config>
            "#,
        );

        let item = item.unwrap();
        assert!(!item.match_path("[$text]"));
    }

    #[test]
    fn test_valid_xml_type_after_tag() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <block>A\B\C</block>|
                </type>
            </config>
            "#,
        );

        let item = dbg!(item).unwrap();
        assert_eq!(item.path, "/config/type");
        assert!(item.tag.is_none());
    }

    #[test]
    fn test_valid_xml_tag_with_underscore() {
        let item = get_test_position_path(
            r#"<?xml version=\"1.0\"?>
            <config>
                <type name="A\B\C">
                    <source_model>asdf|</source_model>
                </type>
            </config>
            "#,
        );

        let item = dbg!(item).unwrap();
        assert!(item.match_path("/source[$text]"));
        assert!(item.attribute_eq("_model", ""));
    }
}
