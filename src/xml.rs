use lsp_types::{Position, Url};
use std::{collections::HashMap, path::Path};
use tree_sitter::{Query, QueryCursor};

use crate::{
    indexer::Indexer,
    js,
    m2_types::M2Item,
    ts::{get_node_text, node_at_position},
};

#[derive(Debug, Clone)]
enum XmlPart {
    Text,
    Attribute(String),
    None,
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

pub fn get_item_from_position(index: &Indexer, uri: &Url, pos: Position) -> Option<M2Item> {
    let path = uri.to_file_path().expect("Should be valid file path");
    let path = path.to_str()?;
    let content = std::fs::read_to_string(path).expect("Should have been able to read the file");
    get_item_from_pos(index, &content, uri, pos)
}

fn get_item_from_pos(index: &Indexer, content: &str, uri: &Url, pos: Position) -> Option<M2Item> {
    let path = uri.to_file_path().expect("Should be valid file path");
    let path = path.to_str()?;
    let tag = get_xml_tag_at_pos(content, pos)?;

    match tag.hover_on {
        XmlPart::Attribute(ref attr_name) => match attr_name.as_str() {
            "method" | "instance" | "class" => try_method_item_from_tag(&tag).or_else(|| {
                try_any_item_from_str(tag.attributes.get(attr_name)?, is_frontend_location(path))
            }),
            "template" => {
                try_phtml_item_from_str(tag.attributes.get(attr_name)?, is_frontend_location(path))
            }
            _ => try_any_item_from_str(tag.attributes.get(attr_name)?, is_frontend_location(path)),
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
                        js::text_to_component(index, text, uri)
                    } else {
                        try_any_item_from_str(text, is_frontend_location(path))
                    }
                }
                _ => try_any_item_from_str(text, is_frontend_location(path)),
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

fn try_any_item_from_str(text: &str, is_frontend: bool) -> Option<M2Item> {
    if does_ext_eq(text, "phtml") {
        try_phtml_item_from_str(text, is_frontend)
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

fn try_phtml_item_from_str(text: &str, is_frontend: bool) -> Option<M2Item> {
    if text.split("::").count() == 2 {
        let mut parts = text.split("::");
        if is_frontend {
            Some(M2Item::FrontPhtml(
                parts.next()?.to_string(),
                parts.next()?.to_string(),
            ))
        } else {
            Some(M2Item::AdminPhtml(
                parts.next()?.to_string(),
                parts.next()?.to_string(),
            ))
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

fn is_frontend_location(path: &str) -> bool {
    path.contains("\\view\\frontend\\")
        || path.contains("/view/frontend/")
        || path.contains("\\app\\design\\frontend\\")
        || path.contains("app/design/frontend/")
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
        let uri = Url::from_file_path(PathBuf::from(if cfg!(windows) { &win_path } else { path }))
            .unwrap();
        let index = Indexer::new(Url::from_file_path("/a/b/c").ok()?);
        get_item_from_pos(&index, &xml.replace('|', ""), &uri, pos)
    }

    #[test]
    fn test_get_item_from_pos_class_in_tag_text() {
        let item = get_test_item(r#"<?xml version="1.0"?><item>|A\B\C</item>"#, "/a/b/c");

        assert_eq!(item, Some(M2Item::Class("A\\B\\C".to_string())));
    }

    #[test]
    fn test_get_item_from_pos_template_in_tag_attribute() {
        let item = get_test_item(
            r#"<?xml version="1.0"?><block template="Some_|Module::path/to/file.phtml"></block>"#,
            "/a/b/c",
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
        let item = get_test_item(
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
        let item = get_test_item(
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
        let item = get_test_item(
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
        let item = get_test_item(
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
        let item = get_test_item(
            r#"<?xml version="1.0"?><service something="\|A\B\C" method="metHod">xx</service>"#,
            "/a/a/c",
        );
        assert_eq!(item, Some(M2Item::Class("A\\B\\C".to_string())));
    }

    #[test]
    fn test_get_item_from_pos_class_in_text_in_tag() {
        let item = get_test_item(r#"<?xml version="1.0"?><some>|A\B\C</some>"#, "/a/a/c");
        assert_eq!(item, Some(M2Item::Class("A\\B\\C".to_string())));
    }

    #[test]
    fn test_get_item_from_pos_const_in_text_in_tag() {
        let item = get_test_item(
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
        let item = get_test_item(
            r#"<?xml version="1.0"?><some>Some_Module::fi|le.phtml</some>"#,
            "/a/a/c",
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
        let item = get_test_item(
            r#"<?xml version="1.0"?><service something="\A\B\C" method="met|Hod">xx</service>"#,
            "/a/a/c",
        );
        assert_eq!(item, None)
    }

    #[test]
    fn test_should_get_most_inner_tag_from_nested() {
        let item = get_test_item(
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
        let item = get_test_item(
            r#"<?xml version=\"1.0\"?>
               <block class="A\|B\C" name="some_name" template="Some_Module::temp/file.phtml"/>
            "#,
            "/a/a/c",
        );
        assert_eq!(item, Some(M2Item::Class("A\\B\\C".to_string())))
    }
}
