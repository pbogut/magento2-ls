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
    m2::{M2Area, M2Item, M2Path},
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
enum Trackee {
    Module(String),
    ModulePath(String),
    JsMap(M2Area, String),
    JsMixin(M2Area, String),
    Themes(M2Area, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrackingList(HashMap<PathBuf, Vec<Trackee>>);

impl TrackingList {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn track(&mut self, source_path: &Path, trackee: Trackee) {
        self.0
            .entry(source_path.into())
            .or_insert_with(Vec::new)
            .push(trackee);
    }

    pub fn maybe_track(&mut self, source_path: Option<&PathBuf>, trackee: Trackee) {
        if let Some(source_path) = source_path {
            self.track(source_path, trackee);
        }
    }

    pub fn untrack(&mut self, source_path: &Path) -> Option<Vec<Trackee>> {
        self.0.remove(source_path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    source_file: Option<PathBuf>,
    track_entities: TrackingList,
    buffers: HashMap<PathBuf, String>,
    modules: Vec<String>,
    module_paths: HashMap<String, PathBuf>,
    front_themes: HashMap<String, PathBuf>,
    admin_themes: HashMap<String, PathBuf>,
    js_maps: [HashMap<String, String>; 3],
    js_mixins: [HashMap<String, Vec<M2Item>>; 3],
    workspaces: Vec<PathBuf>,
}

#[allow(clippy::module_name_repetitions)]
pub type ArcState = Arc<Mutex<State>>;

impl State {
    pub fn new() -> Self {
        Self {
            source_file: None,
            track_entities: TrackingList::new(),
            buffers: HashMap::new(),
            modules: vec![],
            module_paths: HashMap::new(),
            front_themes: HashMap::new(),
            admin_themes: HashMap::new(),
            js_maps: [HashMap::new(), HashMap::new(), HashMap::new()],
            js_mixins: [HashMap::new(), HashMap::new(), HashMap::new()],
            workspaces: vec![],
        }
    }

    pub fn set_source_file(&mut self, path: &Path) {
        self.source_file = Some(path.to_owned());
    }

    pub fn clear_from_source(&mut self, path: &Path) {
        if let Some(list) = self.track_entities.untrack(path) {
            for trackee in list {
                match trackee {
                    Trackee::JsMap(area, name) => {
                        self.js_maps[area.id()].remove(&name);
                    }
                    Trackee::JsMixin(area, name) => {
                        self.js_mixins[area.id()].remove(&name);
                    }
                    Trackee::Module(module) => {
                        self.modules.retain(|m| m != &module);
                    }
                    Trackee::ModulePath(module) => {
                        self.module_paths.remove(&module);
                    }
                    Trackee::Themes(area, module) => match area {
                        M2Area::Frontend => {
                            self.front_themes.remove(&module);
                        }
                        M2Area::Adminhtml => {
                            self.admin_themes.remove(&module);
                        }
                        M2Area::Base => {
                            self.front_themes.remove(&module);
                            self.admin_themes.remove(&module);
                        }
                    },
                }
            }
        }
    }

    pub fn set_file<S>(&mut self, path: &Path, content: S)
    where
        S: Into<String>,
    {
        let content = content.into();
        self.clear_from_source(path);
        js::maybe_index_file(self, &content, &path.to_owned());
        php::maybe_index_file(self, &content, &path.to_owned());

        self.buffers.insert(path.to_owned(), content);
    }

    pub fn get_file(&self, path: &PathBuf) -> Option<&String> {
        self.buffers.get(path)
    }

    pub fn del_file(&mut self, path: &PathBuf) {
        self.buffers.remove(path);
    }

    pub fn get_modules(&self) -> Vec<String> {
        let mut modules = self.modules.clone();
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
        self.module_paths.get(module).cloned()
    }

    pub fn add_module(&mut self, module: &str) -> &mut Self {
        self.track_entities
            .maybe_track(self.source_file.as_ref(), Trackee::Module(module.into()));

        self.modules.push(module.into());
        self
    }

    pub fn add_module_path<S>(&mut self, module: S, path: PathBuf) -> &mut Self
    where
        S: Into<String>,
    {
        let module = module.into();
        self.track_entities.maybe_track(
            self.source_file.as_ref(),
            Trackee::ModulePath(module.clone()),
        );

        self.module_paths.insert(module, path);
        self
    }

    pub fn add_admin_theme_path<S>(&mut self, name: S, path: PathBuf)
    where
        S: Into<String>,
    {
        let name = name.into();
        self.track_entities.maybe_track(
            self.source_file.as_ref(),
            Trackee::Themes(M2Area::Adminhtml, name.clone()),
        );

        self.admin_themes.insert(name, path);
    }

    pub fn add_front_theme_path<S>(&mut self, name: S, path: PathBuf)
    where
        S: Into<String>,
    {
        let name = name.into();
        self.track_entities.maybe_track(
            self.source_file.as_ref(),
            Trackee::Themes(M2Area::Frontend, name.clone()),
        );

        self.front_themes.insert(name, path);
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

    pub fn add_component_map<S>(&mut self, name: S, val: S, area: &M2Area)
    where
        S: Into<String>,
    {
        let name = name.into();
        self.track_entities.maybe_track(
            self.source_file.as_ref(),
            Trackee::JsMap(area.clone(), name.clone()),
        );

        self.js_maps[area.id()].insert(name, val.into());
    }

    pub fn add_component_mixin<S>(&mut self, name: S, val: S, area: &M2Area)
    where
        S: Into<String>,
    {
        let name = name.into();
        let val = val.into();

        self.track_entities.maybe_track(
            self.source_file.as_ref(),
            Trackee::JsMixin(area.clone(), name.clone()),
        );

        if let Some(component) = js::text_to_component(self, &val, Path::new("")) {
            self.js_mixins[area.id()]
                .entry(name)
                .or_insert_with(Vec::new)
                .push(component);
        }
    }

    pub fn get_component_mixins_for_area<S>(&self, name: S, area: &M2Area) -> Vec<M2Item>
    where
        S: Into<String>,
    {
        self.js_mixins[area.id()]
            .get(&name.into())
            .cloned()
            .unwrap_or_default()
    }

    pub fn list_front_themes_paths(&self) -> Vec<&PathBuf> {
        self.front_themes.values().collect::<Vec<&PathBuf>>()
    }

    pub fn list_admin_themes_paths(&self) -> Vec<&PathBuf> {
        self.admin_themes.values().collect::<Vec<&PathBuf>>()
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
        match path.get_ext().as_str() {
            "js" => js::get_item_from_position(self, path, pos),
            "xml" => xml::get_item_from_position(self, path, pos),
            _ => None,
        }
    }

    pub fn into_arc(self) -> ArcState {
        Arc::new(Mutex::new(self))
    }

    pub fn update_index(arc_state: &ArcState, path: &Path) -> Vec<JoinHandle<()>> {
        let mut state = arc_state.lock();
        if state.has_workspace_path(path) {
            vec![]
        } else {
            state.add_workspace_path(path);
            vec![
                spawn_index(arc_state, path, php::update_index, "PHP Indexing"),
                spawn_index(arc_state, path, js::update_index, "JS Indexing"),
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
    state: &ArcState,
    path: &Path,
    callback: fn(&ArcState, &PathBuf),
    msg: &str,
) -> JoinHandle<()> {
    let state = Arc::clone(state);
    let path = path.to_path_buf();
    let msg = msg.to_owned();

    spawn(move || {
        eprintln!("Start {}", msg);
        let index_start = SystemTime::now();
        callback(&state, &path);
        index_start.elapsed().map_or_else(
            |_| eprintln!("{} done", msg),
            |d| eprintln!("{} done in {:?}", msg, d),
        );
    })
}
