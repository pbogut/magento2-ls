use std::path::{Path, PathBuf};

use lsp_types::Location;

use crate::{
    m2::{M2Item, M2Path},
    state::State,
};

use super::path_to_location;

pub fn find_plain(state: &State, comp: &str) -> Vec<Location> {
    let mut result = vec![];
    let workspace_paths = state.workspace_paths();
    for path in workspace_paths {
        let path = path.append(&["lib", "web", comp]).append_ext("js");
        if let Some(location) = path_to_location(&path) {
            result.push(location);
        }
    }
    result
}

pub fn find_rel(comp: String, path: &Path) -> Option<Vec<Location>> {
    let mut path = path.join(comp);
    path.set_extension("js");
    path_to_location(&path).map(|location| vec![location])
}

pub fn mod_location(
    state: &State,
    mod_name: String,
    file_path: &str,
    mod_path: PathBuf,
    path: &PathBuf,
) -> Vec<Location> {
    let mut result = vec![];
    let mut components = vec![M2Item::ModComponent(
        mod_name.clone(),
        file_path.to_string(),
        mod_path,
    )];

    let area = path.get_area();
    components.extend(state.get_component_mixins_for_area(mod_name + "/" + file_path, &area));

    for component in components {
        if let M2Item::ModComponent(_, file_path, mod_path) = component {
            for area_path in area.path_candidates() {
                let comp_path = mod_path
                    .append(&["view", area_path, "web", &file_path])
                    .append_ext("js");
                if let Some(location) = path_to_location(&comp_path) {
                    result.push(location);
                }
            }
        }
    }

    result
}

pub fn mod_html_location(file_path: &str, mod_path: PathBuf, path: &PathBuf) -> Vec<Location> {
    let mut result = vec![];
    let area = path.get_area();
    for area_path in area.path_candidates() {
        let comp_path = mod_path.append(&["view", area_path, "web", &file_path]);
        if let Some(location) = path_to_location(&comp_path) {
            result.push(location);
        }
    }

    result
}
