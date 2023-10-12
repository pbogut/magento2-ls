use lsp_types::Location;

use crate::{
    m2::{M2Area, M2Path},
    state::State,
};

use super::path_to_location;

pub fn find_admin(state: &State, mod_name: &str, template: &str) -> Vec<Location> {
    let mut result = vec![];
    add_phtml_in_mod_location(state, &mut result, mod_name, template, &M2Area::Adminhtml);
    add_phtml_in_admin_theme_location(state, &mut result, mod_name, template);
    result
}

pub fn find_front(state: &State, mod_name: &str, template: &str) -> Vec<Location> {
    let mut result = vec![];
    add_phtml_in_mod_location(state, &mut result, mod_name, template, &M2Area::Frontend);
    add_phtml_in_front_theme_location(state, &mut result, mod_name, template);
    result
}

pub fn find_base(state: &State, mod_name: &str, template: &str) -> Vec<Location> {
    let mut result = vec![];
    add_phtml_in_mod_location(state, &mut result, mod_name, template, &M2Area::Base);
    add_phtml_in_front_theme_location(state, &mut result, mod_name, template);
    add_phtml_in_admin_theme_location(state, &mut result, mod_name, template);
    result
}

fn add_phtml_in_mod_location(
    state: &State,
    result: &mut Vec<Location>,
    mod_name: &str,
    template: &str,
    area: &M2Area,
) {
    let mod_path = state.get_module_path(mod_name);
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
    state: &State,
    result: &mut Vec<Location>,
    mod_name: &str,
    template: &str,
) {
    #[allow(clippy::significant_drop_in_scrutinee)]
    for theme_path in state.list_admin_themes_paths() {
        let path = theme_path.append(&[mod_name, "templates", template]);
        if let Some(location) = path_to_location(&path) {
            result.push(location);
        }
    }
}

fn add_phtml_in_front_theme_location(
    state: &State,
    result: &mut Vec<Location>,
    mod_name: &str,
    template: &str,
) {
    #[allow(clippy::significant_drop_in_scrutinee)]
    for theme_path in state.list_front_themes_paths() {
        let path = theme_path.append(&[mod_name, "templates", template]);
        if let Some(location) = path_to_location(&path) {
            result.push(location);
        }
    }
}
