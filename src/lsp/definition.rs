use std::path::Path;

use lsp_types::{GotoDefinitionParams, Location, Range, Url};
use parking_lot::MutexGuard;

use crate::{
    m2::{M2Area, M2Item, M2Path, M2Uri},
    php::{self, PHPClass},
    state::{ArcState, State},
};

pub fn get_location_from_params(
    state: &ArcState,
    params: &GotoDefinitionParams,
) -> Option<Vec<Location>> {
    let path = params
        .text_document_position_params
        .text_document
        .uri
        .to_path_buf();
    let pos = params.text_document_position_params.position;

    let item = state.lock().get_item_from_position(&path, pos);
    match item {
        Some(M2Item::ModComponent(_mod_name, file_path, mod_path)) => {
            let mut result = vec![];

            for area in [M2Area::Frontend, M2Area::Adminhtml, M2Area::Base] {
                let comp_path = mod_path
                    .append(&["view", area.to_str(), "web", &file_path])
                    .append_ext("js");
                if let Some(location) = path_to_location(&comp_path) {
                    result.push(location);
                }
            }

            Some(result)
        }
        Some(M2Item::RelComponent(comp, path)) => {
            let mut path = path.join(comp);
            path.set_extension("js");
            path_to_location(&path).map(|location| vec![location])
        }
        Some(M2Item::Component(comp)) => {
            dbg!(&comp);
            let mut result = vec![];
            let workspace_paths = state.lock().workspace_paths();
            for path in workspace_paths {
                let path = path.append(&["lib", "web", &comp]).append_ext("js");
                dbg!(&path);
                if let Some(location) = path_to_location(&path) {
                    result.push(location);
                }
            }
            Some(result)
        }
        Some(M2Item::AdminPhtml(mod_name, template)) => {
            let mut result = vec![];
            add_phtml_in_mod_location(state, &mut result, &mod_name, &template, &M2Area::Adminhtml);
            add_phtml_in_admin_theme_location(state, &mut result, &mod_name, &template);
            Some(result)
        }
        Some(M2Item::FrontPhtml(mod_name, template)) => {
            let mut result = vec![];
            add_phtml_in_mod_location(state, &mut result, &mod_name, &template, &M2Area::Frontend);
            add_phtml_in_front_theme_location(state, &mut result, &mod_name, &template);
            Some(result)
        }
        Some(M2Item::BasePhtml(mod_name, template)) => {
            let mut result = vec![];
            add_phtml_in_mod_location(state, &mut result, &mod_name, &template, &M2Area::Base);
            add_phtml_in_front_theme_location(state, &mut result, &mod_name, &template);
            add_phtml_in_admin_theme_location(state, &mut result, &mod_name, &template);
            Some(result)
        }
        Some(M2Item::Class(class)) => {
            let phpclass = get_php_class_from_class_name(&state.lock(), &class)?;
            Some(vec![Location {
                uri: phpclass.uri.clone(),
                range: phpclass.range,
            }])
        }
        Some(M2Item::Method(class, method)) => {
            let phpclass = get_php_class_from_class_name(&state.lock(), &class)?;
            Some(vec![Location {
                uri: phpclass.uri.clone(),
                range: phpclass
                    .methods
                    .get(&method)
                    .map_or(phpclass.range, |method| method.range),
            }])
        }
        Some(M2Item::Const(class, constant)) => {
            let phpclass = get_php_class_from_class_name(&state.lock(), &class)?;
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

fn add_phtml_in_mod_location(
    state: &ArcState,
    result: &mut Vec<Location>,
    mod_name: &str,
    template: &str,
    area: &M2Area,
) {
    let mod_path = state.lock().get_module_path(mod_name);
    if let Some(path) = mod_path {
        for area in area.path_candidates() {
            let templ_path = path.append(&["view", area, "templates", template]);
            if let Some(location) = path_to_location(&templ_path) {
                result.push(location);
            }
        }
    }
}

fn add_phtml_in_admin_theme_location(
    state: &ArcState,
    result: &mut Vec<Location>,
    mod_name: &str,
    template: &str,
) {
    #[allow(clippy::significant_drop_in_scrutinee)]
    for theme_path in state.lock().list_admin_themes_paths() {
        let path = theme_path.append(&[mod_name, "templates", template]);
        if let Some(location) = path_to_location(&path) {
            result.push(location);
        }
    }
}

fn add_phtml_in_front_theme_location(
    state: &ArcState,
    result: &mut Vec<Location>,
    mod_name: &str,
    template: &str,
) {
    #[allow(clippy::significant_drop_in_scrutinee)]
    for theme_path in state.lock().list_front_themes_paths() {
        let path = theme_path.append(&[mod_name, "templates", template]);
        if let Some(location) = path_to_location(&path) {
            result.push(location);
        }
    }
}

fn get_php_class_from_class_name(state: &MutexGuard<State>, class: &str) -> Option<PHPClass> {
    let module_path = state.split_class_to_path_and_suffix(class);
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

fn path_to_location(path: &Path) -> Option<Location> {
    if path.is_file() {
        Some(Location {
            uri: Url::from_file_path(path).expect("Should be valid Url"),
            range: Range::default(),
        })
    } else {
        None
    }
}
