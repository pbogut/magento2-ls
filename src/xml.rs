use std::collections::HashMap;

use crate::ts::*;
use lsp_types::{Location, Position, Url};
use tree_sitter::{Query, QueryCursor};

use crate::php::PHPClass;

pub fn get_location_from_position(
    map: &HashMap<String, PHPClass>,
    uri: Url,
    pos: Position,
) -> Option<Location> {
    let path = uri.path();

    let query_string = "
        (attribute_value) @attr
        (text) @text

        (self_closing_tag (tag_name)
          (attribute (attribute_name ) @_attr2 (#eq? @_attr2 \"class\")
            (quoted_attribute_value (attribute_value) @class))
          ) @callable
        (self_closing_tag (tag_name)
          (attribute (attribute_name) @_attr (#eq? @_attr \"method\")
            (quoted_attribute_value (attribute_value) @method))
          ) @callable
        (self_closing_tag (tag_name) @_name
          (attribute (attribute_name ) @_attr2 (#eq? @_attr2 \"instance\")
            (quoted_attribute_value (attribute_value) @class))
          ) @callable
        (start_tag (tag_name)
          (attribute (attribute_name ) @_attr2 (#eq? @_attr2 \"class\")
            (quoted_attribute_value (attribute_value) @class))
          ) @callable
        (start_tag (tag_name)
          (attribute (attribute_name) @_attr (#eq? @_attr \"method\")
            (quoted_attribute_value (attribute_value) @method))
          ) @callable
        (start_tag (tag_name) @_name
          (attribute (attribute_name ) @_attr2 (#eq? @_attr2 \"instance\")
            (quoted_attribute_value (attribute_value) @class))
          ) @callable
    ";

    let content = std::fs::read_to_string(&path).expect("Should have been able to read the file");

    let tree = tree_sitter_parsers::parse(&content, "html");
    let query = Query::new(tree.language(), &query_string)
        .map_err(|e| eprintln!("Error creating query: {:?}", e))
        .unwrap();

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut class_name: Option<String> = None;
    let mut method_name: Option<String> = None;

    // FIXME its ugly as fuck, figure out better way to get this data
    for m in matches {
        let node = m.captures[0].node;
        if node_at_position(node, pos) {
            if node.kind() == "attribute_value" || node.kind() == "text" {
                class_name = Some(get_node_text(node, &content));
            } else if node.kind() == "self_closing_tag" || node.kind() == "start_tag" {
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if child.kind() == "attribute" {
                        let attr_name = child
                            .named_child(0)
                            .map(|attr| get_node_text(attr, &content))
                            .unwrap_or("".to_string());
                        if attr_name == "class" || attr_name == "instance" {
                            class_name = Some(get_node_text(
                                child.named_child(1)?.named_child(0)?,
                                &content,
                            ));
                        }
                        if attr_name == "method" {
                            method_name = Some(get_node_text(
                                child.named_child(1)?.named_child(0)?,
                                &content,
                            ));
                        }
                    }
                }
            }
        }
    }

    match (class_name, method_name) {
        (Some(class), Some(method)) => map.get(&class).map_or(None, |cls| {
            cls.methods.get(&method).map_or(None, |m| {
                Some(Location {
                    uri: cls.uri.clone(),
                    range: m.range.clone(),
                })
            })
        }),
        (Some(class), None) => map.get(&class).map_or(None, |cls| {
            Some(Location {
                uri: cls.uri.clone(),
                range: cls.range.clone(),
            })
        }),
        _ => None,
    }
}
