use lsp_types::Location;
use parking_lot::MutexGuard;

use crate::{
    php::{parse_php_file, PHPClass},
    state::{ArcState, State},
};

pub fn find_class(state: &ArcState, class: &str) -> Option<Location> {
    let phpclass = get_php_class_from_class_name(&state.lock(), class)?;
    Some(Location {
        uri: phpclass.uri.clone(),
        range: phpclass.range,
    })
}

pub fn find_method(state: &ArcState, class: &str, method: &str) -> Option<Location> {
    let phpclass = get_php_class_from_class_name(&state.lock(), class)?;
    Some(Location {
        uri: phpclass.uri.clone(),
        range: phpclass
            .methods
            .get(method)
            .map_or(phpclass.range, |method| method.range),
    })
}

pub fn find_const(state: &ArcState, class: &str, constant: &str) -> Option<Location> {
    let phpclass = get_php_class_from_class_name(&state.lock(), class)?;
    Some(Location {
        uri: phpclass.uri.clone(),
        range: phpclass
            .constants
            .get(constant)
            .map_or(phpclass.range, |method| method.range),
    })
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
                Ok(true) => parse_php_file(&file_path),
                _ => None,
            }
        }
    }
}
