use std::{path::PathBuf, sync::MutexGuard};

use lsp_types::{GotoDefinitionParams, Location, Range, Url};

use crate::{
    indexer::{ArcIndexer, Indexer},
    js,
    m2_types::M2Item,
    php::{self, PHPClass},
};

pub fn get_location_from_params(
    index: &ArcIndexer,
    params: GotoDefinitionParams,
) -> Option<Vec<Location>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let item = Indexer::lock(index).get_item_from_position(&uri, pos);
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
            let workspace_paths = Indexer::lock(index).workspace_paths();
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
            let mod_path = Indexer::lock(index).get_module_path(&mod_name);
            if let Some(path) = mod_path {
                let templ_path = path
                    .join("view")
                    .join("adminhtml")
                    .join("templates")
                    .join(&template);
                if templ_path.is_file() {
                    result.push(Location {
                        uri: Url::from_file_path(templ_path).expect("Should be valid Url"),
                        range: Range::default(),
                    });
                }
            };

            #[allow(clippy::significant_drop_in_scrutinee)]
            for theme_path in Indexer::lock(index).list_admin_themes_paths() {
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
            let mod_path = Indexer::lock(index).get_module_path(&mod_name);
            if let Some(path) = mod_path {
                let templ_path = path
                    .join("view")
                    .join("frontend")
                    .join("templates")
                    .join(&template);
                if templ_path.is_file() {
                    result.push(Location {
                        uri: Url::from_file_path(templ_path).expect("Should be valid Url"),
                        range: Range::default(),
                    });
                }
            };

            #[allow(clippy::significant_drop_in_scrutinee)]
            for theme_path in Indexer::lock(index).list_front_themes_paths() {
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
            let phpclass = get_php_class_from_class_name(&Indexer::lock(index), &class)?;
            Some(vec![Location {
                uri: phpclass.uri.clone(),
                range: phpclass.range,
            }])
        }
        Some(M2Item::Method(class, method)) => {
            let phpclass = get_php_class_from_class_name(&Indexer::lock(index), &class)?;
            Some(vec![Location {
                uri: phpclass.uri.clone(),
                range: phpclass
                    .methods
                    .get(&method)
                    .map_or(phpclass.range, |method| method.range),
            }])
        }
        Some(M2Item::Const(class, constant)) => {
            let phpclass = get_php_class_from_class_name(&Indexer::lock(index), &class)?;
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
