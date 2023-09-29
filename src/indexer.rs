use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    thread::{spawn, JoinHandle},
    time::SystemTime,
};

use lsp_types::{Position, Url};

use crate::{js, m2_types::M2Item, php, xml};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Indexer {
    magento_modules: HashMap<String, PathBuf>,
    magento_front_themes: HashMap<String, PathBuf>,
    magento_admin_themes: HashMap<String, PathBuf>,
    js_maps: HashMap<String, String>,
    js_mixins: HashMap<String, String>,
    workspaces: Vec<PathBuf>,
}

#[allow(clippy::module_name_repetitions)]
pub type ArcIndexer = Arc<Mutex<Indexer>>;

impl Indexer {
    pub fn new() -> Self {
        Self {
            magento_modules: HashMap::new(),
            magento_front_themes: HashMap::new(),
            magento_admin_themes: HashMap::new(),
            js_maps: HashMap::new(),
            js_mixins: HashMap::new(),
            workspaces: vec![],
        }
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

    pub fn workspace_paths(&self) -> Vec<PathBuf> {
        self.workspaces.clone()
    }

    pub fn add_workspace_path(&mut self, path: &Path) {
        self.workspaces.push(path.to_path_buf());
    }

    pub fn has_workspace_path(&mut self, path: &Path) -> bool {
        self.workspaces.contains(&path.to_path_buf())
    }

    pub fn get_item_from_position(&self, uri: &Url, pos: Position) -> Option<M2Item> {
        let file_path = uri.to_file_path().expect("Should be valid file path");
        match file_path.extension()?.to_str()?.to_lowercase().as_str() {
            "js" => js::get_item_from_position(self, uri, pos),
            "xml" => xml::get_item_from_position(self, uri, pos),
            _ => None,
        }
    }

    pub fn into_arc(self) -> ArcIndexer {
        Arc::new(Mutex::new(self))
    }

    pub fn update_index(arc_index: &ArcIndexer, path: &Path) -> Vec<JoinHandle<()>> {
        let mut lock = Self::lock(arc_index);
        if lock.has_workspace_path(path) {
            vec![]
        } else {
            lock.add_workspace_path(path);
            vec![
                spawn_index(arc_index, path, php::update_index, "PHP Indexing"),
                spawn_index(arc_index, path, js::update_index, "JS Indexing"),
            ]
        }
    }

    pub fn lock(arc_indexer: &ArcIndexer) -> MutexGuard<Self> {
        arc_indexer.lock().expect("Should be able to lock indexer")
    }
}

fn spawn_index(
    arc_indexer: &ArcIndexer,
    path: &Path,
    callback: fn(&ArcIndexer, &Path),
    msg: &str,
) -> JoinHandle<()> {
    let index = Arc::clone(arc_indexer);
    let path = path.to_path_buf();
    let msg = msg.to_owned();

    spawn(move || {
        eprintln!("Start {}", msg);
        let index_start = SystemTime::now();
        callback(&index, &path);
        index_start.elapsed().map_or_else(
            |_| eprintln!("{} done", msg),
            |d| eprintln!("{} done in {:?}", msg, d),
        );
    })
}
