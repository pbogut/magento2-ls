use std::path::PathBuf;

use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Location, Range, Url,
};
use parking_lot::MutexGuard;

use crate::{
    indexer::{ArcIndexer, Indexer},
    js,
    m2_types::{M2Item, M2Path},
    php::{self, PHPClass},
    xml,
};

pub fn completion_handler(indexer: &ArcIndexer, params: CompletionParams) -> CompletionResponse {
    CompletionResponse::Array(
        get_completion_from_params(indexer, params).map_or(vec![], |loc_list| loc_list),
    )
}

pub fn definition_handler(
    indexer: &ArcIndexer,
    params: GotoDefinitionParams,
) -> GotoDefinitionResponse {
    GotoDefinitionResponse::Array(
        get_location_from_params(indexer, params).map_or(vec![], |loc_list| loc_list),
    )
}

pub fn get_location_from_params(
    index: &ArcIndexer,
    params: GotoDefinitionParams,
) -> Option<Vec<Location>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let item = index.lock().get_item_from_position(&uri, pos);
    match item {
        Some(M2Item::ModComponent(_mod_name, file_path, mod_path)) => {
            let mut result = vec![];
            for uri in js::make_web_uris(&mod_path, &PathBuf::from(&file_path)) {
                result.push(Location {
                    uri,
                    range: Range::default(),
                });
            }

            Some(result)
        }
        Some(M2Item::RelComponent(comp, path)) => {
            let mut path = path.join(comp);
            path.set_extension("js");
            if path.exists() {
                Some(vec![Location {
                    uri: Url::from_file_path(path).expect("Should be valid url"),
                    range: Range::default(),
                }])
            } else {
                None
            }
        }
        Some(M2Item::Component(comp)) => {
            let mut result = vec![];
            let workspace_paths = index.lock().workspace_paths();
            for path in workspace_paths {
                let mut path = path.join("lib").join("web").join(&comp);
                path.set_extension("js");
                if path.exists() {
                    result.push(Location {
                        uri: Url::from_file_path(path).expect("Should be valid url"),
                        range: Range::default(),
                    });
                }
            }
            Some(result)
        }
        Some(M2Item::AdminPhtml(mod_name, template)) => {
            let mut result = vec![];
            let mod_path = index.lock().get_module_path(&mod_name);
            if let Some(path) = mod_path {
                let templ_path = path.append(&["view", "adminhtml", "templates", &template]);
                if templ_path.is_file() {
                    result.push(Location {
                        uri: Url::from_file_path(templ_path).expect("Should be valid Url"),
                        range: Range::default(),
                    });
                }
                let templ_path = path.append(&["view", "base", "templates", &template]);
                if templ_path.is_file() {
                    result.push(Location {
                        uri: Url::from_file_path(templ_path).expect("Should be valid Url"),
                        range: Range::default(),
                    });
                }
            };

            #[allow(clippy::significant_drop_in_scrutinee)]
            for theme_path in index.lock().list_admin_themes_paths() {
                let templ_path = theme_path.join(&mod_name).join("templates").join(&template);
                if templ_path.is_file() {
                    result.push(Location {
                        uri: Url::from_file_path(templ_path).expect("Should be valid url"),
                        range: Range::default(),
                    });
                }
            }

            Some(result)
        }
        Some(M2Item::FrontPhtml(mod_name, template)) => {
            let mut result = vec![];
            let mod_path = index.lock().get_module_path(&mod_name);
            if let Some(path) = mod_path {
                let templ_path = path.append(&["view", "frontend", "templates", &template]);
                if templ_path.is_file() {
                    result.push(Location {
                        uri: Url::from_file_path(templ_path).expect("Should be valid Url"),
                        range: Range::default(),
                    });
                }
            };

            #[allow(clippy::significant_drop_in_scrutinee)]
            for theme_path in index.lock().list_front_themes_paths() {
                let templ_path = theme_path.join(&mod_name).join("templates").join(&template);
                if templ_path.is_file() {
                    result.push(Location {
                        uri: Url::from_file_path(templ_path).expect("Should be valid url"),
                        range: Range::default(),
                    });
                }
            }

            Some(result)
        }
        Some(M2Item::Class(class)) => {
            let phpclass = get_php_class_from_class_name(&index.lock(), &class)?;
            Some(vec![Location {
                uri: phpclass.uri.clone(),
                range: phpclass.range,
            }])
        }
        Some(M2Item::Method(class, method)) => {
            let phpclass = get_php_class_from_class_name(&index.lock(), &class)?;
            Some(vec![Location {
                uri: phpclass.uri.clone(),
                range: phpclass
                    .methods
                    .get(&method)
                    .map_or(phpclass.range, |method| method.range),
            }])
        }
        Some(M2Item::Const(class, constant)) => {
            let phpclass = get_php_class_from_class_name(&index.lock(), &class)?;
            Some(vec![Location {
                uri: phpclass.uri.clone(),
                range: phpclass
                    .constants
                    .get(&constant)
                    .map_or(phpclass.range, |method| method.range),
            }])
        }
        None => None,
    }
}

pub fn get_completion_from_params(
    index: &ArcIndexer,
    params: CompletionParams,
) -> Option<Vec<CompletionItem>> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let content = index.lock().get_file(&uri)?.clone();

    if uri.is_xml() {
        let xml_completion =
            xml::get_current_position_path(&content, pos, &xml::PathDepth::Attribute)?;

        match xml_completion.path.as_str() {
            "[@template]" => {
                completion_for_template(index, &xml_completion.text, uri.is_frontend())
            }
            _ => None,
        }
    } else {
        None
    }
}

fn completion_for_template(
    index: &ArcIndexer,
    text: &str,
    is_frontend: bool,
) -> Option<Vec<CompletionItem>> {
    if text.is_empty() || is_part_of_module_name(text) {
        Some(
            index
                .lock()
                .get_modules()
                .iter()
                .map(|module| CompletionItem {
                    label: module.clone(),
                    label_details: None,
                    kind: Some(CompletionItemKind::MODULE),
                    detail: None,
                    ..CompletionItem::default()
                })
                .collect(),
        )
    } else if text.contains("::") {
        let module_name = text.split("::").next()?;
        let path = index.lock().get_module_path(module_name);
        match path {
            None => None,
            Some(path) => {
                let view_path = path.append(&[
                    "view",
                    if is_frontend { "frontend" } else { "adminhtml" },
                    "templates",
                ]);
                let glob_path = view_path.append(&["**", "*.phtml"]);
                let glob_path = glob_path.to_str().unwrap_or_default();
                let view_path = view_path.to_str().unwrap_or_default();
                Some(
                    glob::glob(glob_path)
                        .ok()?
                        .map(|file| CompletionItem {
                            label: file
                                .unwrap_or_default()
                                .relative_to(view_path)
                                .to_path_string(),
                            label_details: None,
                            kind: Some(CompletionItemKind::FILE),
                            detail: None,
                            ..CompletionItem::default()
                        })
                        .collect(),
                )
            }
        }
    } else {
        None
    }
}

fn get_php_class_from_class_name(index: &MutexGuard<Indexer>, class: &str) -> Option<PHPClass> {
    let module_path = get_module_path(index, class);
    match module_path {
        None => None,
        Some((mut file_path, suffix)) => {
            for part in suffix {
                file_path.push(part);
            }
            file_path.set_extension("php");

            match file_path.try_exists() {
                Ok(true) => php::parse_php_file(&file_path),
                _ => None,
            }
        }
    }
}

fn get_module_path(index: &MutexGuard<Indexer>, class: &str) -> Option<(PathBuf, Vec<String>)> {
    let mut parts = class.split('\\').collect::<Vec<_>>();
    let mut suffix = vec![];

    while let Some(part) = parts.pop() {
        suffix.push(part.to_string());
        let prefix = parts.join("\\");

        let module_path = index.get_module_path(&prefix);
        match module_path {
            Some(mod_path) => {
                suffix.reverse();
                return Some((mod_path, suffix));
            }
            None => continue,
        }
    }

    None
}

fn is_part_of_module_name(text: &str) -> bool {
    text.chars()
        .reduce(|a, b| {
            if b.is_alphanumeric() || b == '_' && (a != 'N') {
                'Y'
            } else {
                'N'
            }
        })
        .unwrap_or_default()
        == 'Y'
}
