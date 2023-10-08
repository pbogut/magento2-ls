mod component;
mod php;
mod phtml;

use std::path::Path;

use lsp_types::{GotoDefinitionParams, Location, Range, Url};

use crate::{
    m2::{M2Item, M2Uri},
    state::ArcState,
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
    let item = state.lock().get_item_from_position(&path, pos)?;
    Some(match item {
        M2Item::ModComponent(mod_name, file_path, mod_path) => {
            component::mod_location(state, mod_name, &file_path, mod_path, &path)
        }
        M2Item::RelComponent(comp, path) => component::find_rel(comp, &path)?,
        M2Item::Component(comp) => component::find_plain(state, &comp),
        M2Item::AdminPhtml(mod_name, template) => phtml::find_admin(state, &mod_name, &template),
        M2Item::FrontPhtml(mod_name, template) => phtml::find_front(state, &mod_name, &template),
        M2Item::BasePhtml(mod_name, template) => phtml::find_base(state, &mod_name, &template),
        M2Item::Class(class) => vec![php::find_class(state, &class)?],
        M2Item::Method(class, method) => vec![php::find_method(state, &class, &method)?],
        M2Item::Const(class, constant) => vec![php::find_const(state, &class, &constant)?],
    })
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
