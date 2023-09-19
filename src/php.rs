use std::{collections::HashMap, path::PathBuf};

use glob::glob;
use lsp_types::{Position, Range, Url};
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Query, QueryCursor};

use crate::ts::get_range_from_node;

const CACHE_FILE: &str = ".magento2-ls.index";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PHPClass {
    pub fqn: String,
    pub uri: Url,
    pub range: Range,
    pub methods: HashMap<String, PHPMethod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PHPMethod {
    pub name: String,
    pub range: Range,
}

pub fn parse_php_files(map: &mut HashMap<String, PHPClass>, root_path: PathBuf) {
    let vendor_map = load_vendor(&root_path);

    let path_str = root_path
        .to_str()
        .expect("Correct path is required")
        .to_string();

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
                            let path_str = path.to_str().unwrap_or("");
                            if path_str.ends_with("Test.php") {
                                continue;
                            }
                            if path_str.contains("/dev/tests/") {
                                continue;
                            }
                            if path_str.contains("/vendor/") {
                                if let Some(cls) = vendor_map.get(path_str) {
                                    map.insert(cls.fqn.clone(), cls.clone());
                                    continue;
                                }
                            }
                            if path.is_file() {
                                if false {
                                    continue;
                                }
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
    save_vendor(root_path, map);
}

fn load_vendor(root_path: &PathBuf) -> HashMap<String, PHPClass> {
    let root_string = root_path
        .to_str()
        .expect("root path is required")
        .to_string();
    std::fs::File::open(root_string.clone() + "/" + CACHE_FILE).map_or(HashMap::new(), |f| {
        let reader = std::io::BufReader::new(f);
        bincode::deserialize_from(reader).unwrap_or(HashMap::new())
    })
}

fn save_vendor(root_path: PathBuf, map: &HashMap<String, PHPClass>) {
    let root_string = root_path
        .to_str()
        .expect("root path is required")
        .to_string();
    if let Ok(f) = std::fs::File::create(root_string.clone() + "/" + CACHE_FILE) {
        let mut vendor_map: HashMap<String, PHPClass> = HashMap::new();

        for (_, value) in map {
            if value
                .uri
                .path()
                .starts_with((root_string.clone() + "/vendor/").as_str())
            {
                vendor_map.insert(value.uri.path().to_string(), value.clone());
            }
        }
        bincode::serialize_into(f, &vendor_map)
            .map_err(|e| eprintln!("Failed to write index file: {:?}", e))
            .expect("Failed to write index file");
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
    let mut methods: HashMap<String, PHPMethod> = HashMap::new();

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
