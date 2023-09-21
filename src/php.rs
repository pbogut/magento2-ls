use crate::ts::get_range_from_node;
use convert_case::{Case, Casing};
use glob::glob;
use lsp_types::{Position, Range, Url};
use std::{collections::HashMap, path::PathBuf};
use tree_sitter::{Node, Query, QueryCursor};

#[derive(Debug, Clone)]
pub struct Callable {
    pub class: String,
    pub method: Option<String>,
}

#[derive(Debug, Clone)]
pub enum M2Item {
    Class(String),
    Method(String, String),
    Const(String, String),
}

#[derive(Debug, Clone)]
enum M2Module {
    Theme(String),
    Module(String),
    Library(String),
}

#[derive(Debug, Clone)]
pub struct PHPClass {
    pub fqn: String,
    pub uri: Url,
    pub range: Range,
    pub methods: HashMap<String, PHPMethod>,
    pub constants: HashMap<String, PHPConst>,
}

#[derive(Debug, Clone)]
pub struct PHPMethod {
    pub name: String,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct PHPConst {
    pub name: String,
    pub range: Range,
}

fn register_param_to_module(param: &str) -> Option<M2Module> {
    if param.matches('/').count() == 2 {
        Some(M2Module::Theme(param.to_string()))
    } else if param.matches('/').count() == 1 {
        let mut parts = param.splitn(2, '/');
        let p1 = parts.next()?.to_case(Case::Pascal);
        let p2 = parts.next()?;

        if p2.matches('-').count() > 0 {
            let mut parts = p2.splitn(2, '-');
            let p2 = parts.next()?.to_case(Case::Pascal);
            let p3 = parts.next()?.to_case(Case::Pascal);
            Some(M2Module::Library(format!("{}\\{}\\{}", p1, p2, p3)))
        } else {
            Some(M2Module::Library(format!(
                "{}\\{}",
                p1,
                p2.to_case(Case::Pascal)
            )))
        }
    } else if param.matches('_').count() == 1 {
        let mut parts = param.split('_');
        Some(M2Module::Module(format!(
            "{}\\{}",
            parts.next()?,
            parts.next()?
        )))
    } else {
        None
    }
}

pub fn get_modules_map(root_path: &PathBuf) -> HashMap<String, PathBuf> {
    let mut map: HashMap<String, PathBuf> = HashMap::new();
    let modules = glob(root_path.join("**/registration.php").to_str().unwrap())
        .expect("Failed to read glob pattern");

    let module_name_query = "
    (scoped_call_expression
      (name) @reg (#eq? @reg register)
      (arguments
        (string) @module_name
      )
    )";

    for moule_registration in modules {
        moule_registration.map_or_else(
            |_e| panic!("buhu"),
            |file_path| {
                let content = std::fs::read_to_string(&file_path)
                    .expect("Should have been able to read the file");

                let tree = tree_sitter_parsers::parse(&content, "php");
                Query::new(tree.language(), &module_name_query).map_or_else(
                    |e| eprintln!("Error creating query: {:?}", e),
                    |query| {
                        let mut cursor = QueryCursor::new();
                        let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
                        for m in matches {
                            let mod_name = crate::ts::get_node_text(m.captures[1].node, &content);
                            let mod_name = mod_name.trim_matches('"').trim_matches('\'');

                            let mut parent = file_path.clone();
                            parent.pop();

                            match register_param_to_module(mod_name) {
                                Some(M2Module::Module(m)) => {
                                    map.insert(m, parent);
                                }
                                Some(M2Module::Library(l)) => {
                                    map.insert(l, parent);
                                }
                                _ => (),
                            }
                        }
                    },
                );
            },
        );
    }

    map
}

pub fn parse_php_file(file_path: PathBuf) -> Option<PHPClass> {
    let query_string = "
      (namespace_definition (namespace_name) @namespace) ; pattern: 0
      (class_declaration (name) @class)                  ; pattern: 1
      (interface_declaration (name) @class)              ; pattern: 2
      ((method_declaration (visibility_modifier)
        @_vis (name) @name) (#eq? @_vis \"public\"))     ; pattern: 3
      (const_element (name) @const)                      ; pattern: 4
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
    let mut methods: HashMap<String, PHPMethod> = HashMap::new();
    let mut constants: HashMap<String, PHPConst> = HashMap::new();

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
                methods.insert(
                    method_name.to_string(),
                    PHPMethod {
                        name: method_name.to_string(),
                        range: get_range_from_node(method_node),
                    },
                );
            }
        }
        if m.pattern_index == 4 {
            let const_node = m.captures[0].node;
            let const_name = const_node.utf8_text(&content.as_bytes()).unwrap_or("");
            if const_name != "" {
                constants.insert(
                    const_name.to_string(),
                    PHPConst {
                        name: const_name.to_string(),
                        range: get_range_from_node(const_node),
                    },
                );
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
        constants,
    });
}
