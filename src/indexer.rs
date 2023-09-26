use std::{collections::HashMap, path::PathBuf};

use lsp_types::{Position, Url};

use crate::{js, m2_types::M2Item, php, xml};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Indexer {
    magento_modules: HashMap<String, PathBuf>,
    magento_front_themes: HashMap<String, PathBuf>,
    magento_admin_themes: HashMap<String, PathBuf>,
    js_maps: HashMap<String, String>,
    js_mixins: HashMap<String, String>,
    root_uri: Url,
}

impl Indexer {
    pub fn new(root_uri: Url) -> Self {
        Self {
            magento_modules: HashMap::new(),
            magento_front_themes: HashMap::new(),
            magento_admin_themes: HashMap::new(),
            js_maps: HashMap::new(),
            js_mixins: HashMap::new(),
            root_uri,
        }
    }

    pub fn update_index(&mut self) {
        php::update_index(self);
        js::update_index(self);
    }

    pub fn get_module_path(&self, module: &str) -> Option<PathBuf> {
        self.magento_modules.get(module).cloned()
    }

    pub fn add_module_path(&mut self, module: &str, path: PathBuf) {
        self.magento_modules.insert(module.to_string(), path);
    }

    pub fn add_admin_theme_path(&mut self, name: &str, path: PathBuf) {
        self.magento_admin_themes.insert(name.to_string(), path);
    }

    pub fn add_front_theme_path(&mut self, name: &str, path: PathBuf) {
        self.magento_front_themes.insert(name.to_string(), path);
    }

    pub fn get_component_map(&self, name: &str) -> Option<&String> {
        self.js_maps.get(name)
    }

    pub fn add_component_map<S>(&mut self, name: &str, val: S) -> Option<String>
    where
        S: Into<String>,
    {
        self.js_maps.insert(name.to_string(), val.into())
    }

    pub fn add_component_mixin<S>(&mut self, name: &str, val: S) -> Option<String>
    where
        S: Into<String>,
    {
        self.js_mixins.insert(name.to_string(), val.into())
    }

    pub fn list_front_themes_paths(&self) -> Vec<&PathBuf> {
        self.magento_front_themes
            .values()
            .collect::<Vec<&PathBuf>>()
    }

    pub fn list_admin_themes_paths(&self) -> Vec<&PathBuf> {
        self.magento_admin_themes
            .values()
            .collect::<Vec<&PathBuf>>()
    }

    pub fn root_path(&self) -> PathBuf {
        self.root_uri.to_file_path().expect("Invalid root path")
    }

    pub fn get_item_from_position(&self, uri: &Url, pos: Position) -> Option<M2Item> {
        let file_path = uri.to_file_path().expect("Should be valid file path");
        match file_path.extension()?.to_str()?.to_lowercase().as_str() {
            "js" => js::get_item_from_position(self, uri, pos),
            "xml" => xml::get_item_from_position(uri, pos),
            _ => None,
        }
    }
}
