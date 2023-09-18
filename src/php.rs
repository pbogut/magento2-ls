use std::{collections::HashMap, path::PathBuf};

use glob::glob;
use lsp_types::{Position, Range, Url};
use tree_sitter::{Node, Query, QueryCursor};

use crate::ts::get_range_from_node;

#[derive(Debug)]
pub struct PHPClass {
    pub fqn: String,
    pub uri: Url,
    pub range: Range,
    pub methods: Vec<PHPMethod>,
}

#[derive(Debug)]
pub struct PHPMethod {
    pub name: String,
    pub range: Range,
}

pub fn parse_php_files(map: &mut HashMap<String, PHPClass>, path: PathBuf) {
    let path_str = path.to_str().expect("Correct path is required").to_string();
    let tmp_modules =
        glob((path_str + "/**/registration.php").as_str()).expect("Failed to read glob pattern");

    let mut progress_max = 0;
    let mut modules = vec![];
    for module in tmp_modules {
        progress_max += 1;
        modules.push(module);
    }
    let mut progress_cur = 0;
    for module in modules {
        progress_cur += 1;
        eprintln!("Index Progress: {}/{}", progress_cur, progress_max);
        match module {
            Ok(path) => {
                let path_str = path.to_str().expect("path error");
                let files = glob(
                    (path_str[..path_str.len() - "/registration.php".len()].to_string()
                        + "/**/*.php")
                        .as_str(),
                )
                .expect("Failed to read glob pattern");
                for file in files {
                    match file {
                        Ok(path) => {
                            if path.to_str().unwrap_or("").ends_with("Test.php") {
                                continue;
                            }
                            if path.to_str().unwrap_or("").contains("/dev/tests/") {
                                continue;
                            }
                            // TODO get from settings or somethign
                            if path.to_str().unwrap_or("").contains("/vendor/") {
                                continue;
                            }
                            if path.is_file() {
                                match parse_php_file(path) {
                                    Some(cls) => {
                                        map.insert(cls.fqn.clone(), cls);
                                    }
                                    None => {}
                                }
                            }
                        }
                        Err(e) => eprintln!("{:?}", e),
                    }
                }
            }
            Err(e) => eprintln!("{:?}", e),
        }
    }
}

pub fn parse_php_file(file_path: PathBuf) -> Option<PHPClass> {
    let query_string = "
      (namespace_definition (namespace_name) @namespace) ; pattern: 0
      (class_declaration (name) @class)                  ; pattern: 1
      (interface_declaration (name) @class)              ; pattern: 2
      ((method_declaration (visibility_modifier)
        @_vis (name) @name) (#eq? @_vis \"public\"))       ; pattern: 3
    ";

    let content =
        std::fs::read_to_string(&file_path).expect("Should have been able to read the file");

    let tree = tree_sitter_parsers::parse(&content, "php");
    let query = Query::new(tree.language(), &query_string)
        .map_err(|e| eprintln!("Error creating query: {:?}", e))
        .unwrap();

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut ns: Option<Node> = None;
    let mut cls: Option<Node> = None;
    let mut methods: Vec<PHPMethod> = vec![];

    for m in matches {
        if m.pattern_index == 0 {
            ns = Some(m.captures[0].node);
        }
        if m.pattern_index == 1 || m.pattern_index == 2 {
            cls = Some(m.captures[0].node);
        }
        if m.pattern_index == 3 {
            let method_node = m.captures[1].node;
            let method_name = method_node.utf8_text(&content.as_bytes()).unwrap_or("");
            if method_name != "" {
                methods.push(PHPMethod {
                    name: method_name.to_string(),
                    range: get_range_from_node(method_node),
                });
            }
        }
    }

    if ns.is_none() || cls.is_none() {
        return None;
    }

    let ns_node = ns.expect("ns is some");
    let cls_node = cls.expect("cls is some");
    let ns_text = ns_node.utf8_text(&content.as_bytes()).unwrap_or("");
    let cls_text = cls_node.utf8_text(&content.as_bytes()).unwrap_or("");

    let fqn = ns_text.to_string() + "\\" + cls_text;
    if fqn == "\\" {
        return None;
    }

    let uri = Url::from_file_path(file_path.clone()).unwrap();
    let range = Range {
        start: Position {
            line: cls_node.start_position().row as u32,
            character: cls_node.start_position().column as u32,
        },
        end: Position {
            line: cls_node.end_position().row as u32,
            character: cls_node.end_position().column as u32,
        },
    };

    return Some(PHPClass {
        fqn,
        uri,
        range,
        methods,
    });
}
