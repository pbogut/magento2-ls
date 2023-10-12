use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use convert_case::{Case, Casing};
use glob::glob;
use lsp_types::{Position, Range, Url};
use tree_sitter::{Node, QueryCursor};

use crate::{
    m2::M2Path,
    queries,
    state::{ArcState, State},
    ts::{self, get_range_from_node},
};

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

#[derive(Debug, Clone)]
enum M2Module {
    Module(String),
    Library(String),
    FrontTheme(String),
    AdminTheme(String),
}

fn register_param_to_module(param: &str) -> Option<M2Module> {
    if param.matches('/').count() == 2 {
        if param.starts_with("frontend") {
            Some(M2Module::FrontTheme(param.into()))
        } else {
            Some(M2Module::AdminTheme(param.into()))
        }
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

pub fn update_index(state: &ArcState, path: &PathBuf) {
    // if current workspace is magento module
    process_glob(state, &path.append(&["registration.php"]));
    // if current workspace is magento installation
    process_glob(
        state,
        &path.append(&["vendor", "*", "*", "registration.php"]),
    ); // vendor modules / themes
    process_glob(
        state,
        &path.append(&["app", "code", "*", "*", "registration.php"]),
    ); // local modules
    process_glob(
        state,
        &path.append(&["app", "design", "*", "*", "*", "registration.php"]),
    ); // local themes
    process_glob(
        state,
        &path.append(&[
            "vendor",
            "magento",
            "magento2-base",
            "setup",
            "src",
            "Magento",
            "Setup",
            "registration.php",
        ]),
    ); // magento2-base setup module
}

pub fn maybe_index_file(state: &mut State, content: &str, file_path: &PathBuf) {
    if file_path.to_path_str().ends_with("registration.php") {
        update_index_from_registration(state, content, file_path);
    }
}

fn update_index_from_registration(state: &mut State, content: &str, file_path: &Path) {
    state.set_source_file(file_path);
    let query = queries::php_registration();
    let tree = tree_sitter_parsers::parse(content, "php");
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(query, tree.root_node(), content.as_bytes());
    for m in matches {
        let mod_name = ts::get_node_str(m.captures[1].node, content)
            .trim_matches('"')
            .trim_matches('\'');

        let mut parent = file_path.to_path_buf();
        parent.pop();

        state.add_module_path(mod_name, parent.clone());

        match register_param_to_module(mod_name) {
            Some(M2Module::Module(m)) => {
                state.add_module(mod_name).add_module_path(m, parent);
            }
            Some(M2Module::Library(l)) => {
                state
                    .add_module(&l.replace('\\', "_"))
                    .add_module_path(l, parent);
            }
            Some(M2Module::FrontTheme(t)) => {
                state.add_front_theme_path(t, parent);
            }
            Some(M2Module::AdminTheme(t)) => {
                state.add_admin_theme_path(t, parent);
            }
            _ => (),
        }
    }
}

fn process_glob(state: &ArcState, glob_path: &PathBuf) {
    let modules = glob(glob_path.to_path_str())
        .expect("Failed to read glob pattern")
        .filter_map(Result::ok);

    for file_path in modules {
        if file_path.is_test() {
            return;
        }

        let content =
            std::fs::read_to_string(&file_path).expect("Should have been able to read the file");

        update_index_from_registration(&mut state.lock(), &content, &file_path);
    }
}

pub fn parse_php_file(file_path: &PathBuf) -> Option<PHPClass> {
    let content =
        std::fs::read_to_string(file_path).expect("Should have been able to read the file");
    let tree = tree_sitter_parsers::parse(&content, "php");
    let query = queries::php_class();

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(query, tree.root_node(), content.as_bytes());

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
            let method_name = ts::get_node_str(method_node, &content);
            if !method_name.is_empty() {
                methods.insert(
                    method_name.into(),
                    PHPMethod {
                        name: method_name.into(),
                        range: get_range_from_node(method_node),
                    },
                );
            }
        }
        if m.pattern_index == 4 {
            let const_node = m.captures[0].node;
            let const_name = const_node.utf8_text(content.as_bytes()).unwrap_or("");
            if !const_name.is_empty() {
                constants.insert(
                    const_name.into(),
                    PHPConst {
                        name: const_name.into(),
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
    let ns_text = ns_node.utf8_text(content.as_bytes()).unwrap_or("");
    let cls_text = cls_node.utf8_text(content.as_bytes()).unwrap_or("");

    let fqn = ns_text.to_string() + "\\" + cls_text;
    if fqn == "\\" {
        return None;
    }

    let uri = Url::from_file_path(file_path.clone()).expect("Path can not be converted to Url");
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

    Some(PHPClass {
        fqn,
        uri,
        range,
        methods,
        constants,
    })
}
