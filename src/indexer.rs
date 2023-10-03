use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    thread::{spawn, JoinHandle},
    time::SystemTime,
};

use lsp_types::Position;
use parking_lot::Mutex;

use crate::{
    js,
    m2::{M2Area, M2Item},
    php, xml,
};

trait HashMapId {
    fn id(&self) -> usize;
}

impl HashMapId for M2Area {
    fn id(&self) -> usize {
        match self {
            Self::Frontend => 0,
            Self::Adminhtml => 1,
            Self::Base => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Indexer {
    buffers: HashMap<PathBuf, String>,
    magento_modules: Vec<String>,
    magento_module_paths: HashMap<String, PathBuf>,
    magento_front_themes: HashMap<String, PathBuf>,
    magento_admin_themes: HashMap<String, PathBuf>,
    js_maps: [HashMap<String, String>; 3],
    js_mixins: HashMap<String, String>,
    workspaces: Vec<PathBuf>,
}

#[allow(clippy::module_name_repetitions)]
pub type ArcIndexer = Arc<Mutex<Indexer>>;

impl Indexer {
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
            magento_modules: vec![],
            magento_module_paths: HashMap::new(),
            magento_front_themes: HashMap::new(),
            magento_admin_themes: HashMap::new(),
            js_maps: [HashMap::new(), HashMap::new(), HashMap::new()],
            js_mixins: HashMap::new(),
            workspaces: vec![],
        }
    }

    pub fn set_file<S>(&mut self, path: &Path, content: S)
    where
        S: Into<String>,
    {
        self.buffers.insert(path.to_owned(), content.into());
    }

    pub fn get_file(&self, path: &PathBuf) -> Option<&String> {
        self.buffers.get(path)
    }

    pub fn del_file(&mut self, path: &PathBuf) {
        self.buffers.remove(path);
    }

    pub fn get_modules(&self) -> Vec<String> {
        let mut modules = self.magento_modules.clone();
        modules.sort_unstable();
        modules.dedup();
        modules
    }

    pub fn get_module_class_prefixes(&self) -> Vec<String> {
        self.get_modules()
            .iter()
            .map(|m| m.replace('_', "\\"))
            .collect()
    }

    pub fn get_module_path(&self, module: &str) -> Option<PathBuf> {
        self.magento_module_paths.get(module).cloned()
    }

    pub fn add_module(&mut self, module: &str) -> &mut Self {
        self.magento_modules.push(module.to_string());
        self
    }

    pub fn add_module_path(&mut self, module: &str, path: PathBuf) -> &mut Self {
        self.magento_module_paths.insert(module.to_string(), path);
        self
    }

    pub fn add_admin_theme_path(&mut self, name: &str, path: PathBuf) {
        self.magento_admin_themes.insert(name.to_string(), path);
    }

    pub fn add_front_theme_path(&mut self, name: &str, path: PathBuf) {
        self.magento_front_themes.insert(name.to_string(), path);
    }

    pub fn get_component_map(&self, name: &str, area: &M2Area) -> Option<&String> {
        self.js_maps[area.id()].get(name)
    }

    pub fn get_component_maps_for_area(&self, area: &M2Area) -> Vec<String> {
        self.js_maps[area.id()]
            .keys()
            .map(ToString::to_string)
            .collect()
    }

    pub fn add_component_map<S>(&mut self, name: &str, val: S, area: &M2Area) -> Option<String>
    where
        S: Into<String>,
    {
        self.js_maps[area.id()].insert(name.to_string(), val.into())
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

    pub fn get_item_from_position(&self, path: &PathBuf, pos: Position) -> Option<M2Item> {
        match path.extension()?.to_str()?.to_lowercase().as_str() {
            "js" => js::get_item_from_position(self, path, pos),
            "xml" => xml::get_item_from_position(self, path, pos),
            _ => None,
        }
    }

    pub fn into_arc(self) -> ArcIndexer {
        Arc::new(Mutex::new(self))
    }

    pub fn update_index(arc_index: &ArcIndexer, path: &Path) -> Vec<JoinHandle<()>> {
        let mut index = arc_index.lock();
        if index.has_workspace_path(path) {
            vec![]
        } else {
            index.add_workspace_path(path);
            vec![
                spawn_index(arc_index, path, php::update_index, "PHP Indexing"),
                spawn_index(arc_index, path, js::update_index, "JS Indexing"),
            ]
        }
    }

    pub fn split_class_to_path_and_suffix(&self, class: &str) -> Option<(PathBuf, Vec<String>)> {
        let mut parts = class.split('\\').collect::<Vec<_>>();
        let mut suffix = vec![];

        while let Some(part) = parts.pop() {
            suffix.push(part.to_string());
            let prefix = parts.join("\\");
            let module_path = self.get_module_path(&prefix);
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
}

fn spawn_index(
    arc_indexer: &ArcIndexer,
    path: &Path,
    callback: fn(&ArcIndexer, &PathBuf),
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
