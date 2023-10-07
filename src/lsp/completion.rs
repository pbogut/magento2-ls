mod events;

use std::path::PathBuf;

use glob::glob;
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionTextEdit, Position, Range,
    TextEdit,
};

use crate::{
    js::{self, JsCompletionType},
    m2::{self, M2Area, M2Path, M2Uri},
    state::ArcState,
    xml,
};

pub fn get_completion_from_params(
    state: &ArcState,
    params: &CompletionParams,
) -> Option<Vec<CompletionItem>> {
    let path = params
        .text_document_position
        .text_document
        .uri
        .to_path_buf();
    let pos = params.text_document_position.position;
    let content = state.lock().get_file(&path)?.clone();

    match path.get_ext().as_str() {
        "xml" => xml_completion_handler(state, &content, &path, pos),
        "js" => js_completion_handler(state, &content, &path, pos),
        _ => None,
    }
}

fn js_completion_handler(
    state: &ArcState,
    content: &str,
    path: &PathBuf,
    pos: Position,
) -> Option<Vec<CompletionItem>> {
    let at_position = js::get_completion_item(content, pos)?;

    match at_position.kind {
        JsCompletionType::Definition => completion_for_component(
            state,
            &at_position.text,
            at_position.range,
            &path.get_area(),
        ),
    }
}

fn xml_completion_handler(
    state: &ArcState,
    content: &str,
    path: &PathBuf,
    pos: Position,
) -> Option<Vec<CompletionItem>> {
    let at_position = xml::get_current_position_path(content, pos)?;
    match at_position {
        x if x.match_path("[@template]") => {
            completion_for_template(state, &x.text, x.range, &path.get_area())
        }
        x if x.attribute_eq("xsi:type", "string") && x.attribute_eq("name", "template") => {
            completion_for_template(state, &x.text, x.range, &path.get_area())
        }
        x if x.attribute_eq("xsi:type", "string") && x.attribute_eq("name", "component") => {
            completion_for_component(state, &x.text, x.range, &path.get_area())
        }
        x if x.match_path("/config/event[@name]") && path.ends_with("events.xml") => {
            Some(events::get_completion_items(x.range))
        }
        x if x.match_path("/config/preference[@for]") && path.ends_with("di.xml") => {
            completion_for_classes(state, &x.text, x.range)
        }
        x if x.match_path("/config/preference[@type]") && path.ends_with("di.xml") => {
            completion_for_classes(state, &x.text, x.range)
        }
        x if x.match_path("/virtualType[@type]") && path.ends_with("di.xml") => {
            completion_for_classes(state, &x.text, x.range)
        }
        x if x.match_path("[@class]") || x.match_path("[@instance]") => {
            completion_for_classes(state, &x.text, x.range)
        }
        x if x.attribute_in("xsi:type", &["object", "const", "init_parameter"]) => {
            completion_for_classes(state, &x.text, x.range)
        }
        x if x.match_path("/type[@name]") => completion_for_classes(state, &x.text, x.range),
        // Should be /source_model[$text], but html parser dont like undersores
        x if x.match_path("/source[$text]") && x.attribute_eq("_model", "") => {
            completion_for_classes(state, &x.text, x.range)
        }
        // Should be /backend_model[$text], but html parser dont like undersores
        x if x.match_path("/backend[$text]") && x.attribute_eq("_model", "") => {
            completion_for_classes(state, &x.text, x.range)
        }
        // Should be /frontend_model[$text], but html parser dont like undersores
        x if x.match_path("/frontend[$text]") && x.attribute_eq("_model", "") => {
            completion_for_classes(state, &x.text, x.range)
        }
        _ => None,
    }
}

fn completion_for_classes(
    state: &ArcState,
    text: &str,
    range: Range,
) -> Option<Vec<CompletionItem>> {
    let text = text.trim_start_matches('\\');
    if text.is_empty() || (m2::is_part_of_class_name(text) && text.matches('\\').count() == 0) {
        Some(completion_for_classes_prefix(state, range))
    } else if text.matches('\\').count() == 1 {
        let mut result = completion_for_classes_prefix(state, range);
        if let Some(classes) = completion_for_classes_full(state, text, range) {
            result.extend(classes);
        }
        Some(result)
    } else if text.matches('\\').count() >= 2 {
        completion_for_classes_full(state, text, range)
    } else {
        None
    }
}

fn completion_for_classes_prefix(state: &ArcState, range: Range) -> Vec<CompletionItem> {
    let module_prefixes = state.lock().get_module_class_prefixes();
    string_vec_and_range_to_completion_list(module_prefixes, range)
}

fn completion_for_classes_full(
    state: &ArcState,
    text: &str,
    range: Range,
) -> Option<Vec<CompletionItem>> {
    let mut parts = text.split('\\');

    let module_name = format!("{}_{}", parts.next()?, parts.next()?);

    let mut parts = text.split('\\').collect::<Vec<&str>>();
    parts.pop();
    let typed_class_prefix = parts.join("\\");

    let module_class = module_name.replace('_', "\\");
    let module_path = state.lock().get_module_path(&module_name)?;
    let candidates = glob(module_path.append(&["**", "*.php"]).to_path_str())
        .expect("Failed to read glob pattern");
    let mut classes = vec![];
    for p in candidates {
        let path = p.map_or_else(|_| std::path::PathBuf::new(), |p| p);
        let rel_path = path.relative_to(&module_path).str_components().join("\\");
        let class_suffix = rel_path.trim_end_matches(".php");
        let class = format!("{}\\{}", &module_class, class_suffix);

        if class.ends_with("\\registration") {
            continue;
        }

        if !class.starts_with(&typed_class_prefix) {
            continue;
        }

        classes.push(class);
    }
    Some(string_vec_and_range_to_completion_list(classes, range))
}

fn completion_for_template(
    state: &ArcState,
    text: &str,
    range: Range,
    area: &M2Area,
) -> Option<Vec<CompletionItem>> {
    if text.is_empty() || m2::is_part_of_module_name(text) {
        let modules = state.lock().get_modules();
        Some(string_vec_and_range_to_completion_list(modules, range))
    } else if text.contains("::") {
        let module_name = text.split("::").next()?;
        let path = state.lock().get_module_path(module_name);
        match path {
            None => None,
            Some(path) => {
                let mut files = vec![];
                for area_string in area.path_candidates() {
                    let view_path = path.append(&["view", area_string, "templates"]);
                    let glob_path = view_path.append(&["**", "*.phtml"]);
                    files.extend(glob::glob(glob_path.to_path_str()).ok()?.map(|file| {
                        let path = file
                            .unwrap_or_default()
                            .relative_to(&view_path)
                            .str_components()
                            .join("/");
                        String::from(module_name) + "::" + &path
                    }));
                }
                Some(string_vec_and_range_to_completion_list(files, range))
            }
        }
    } else {
        None
    }
}

fn completion_for_component(
    state: &ArcState,
    text: &str,
    range: Range,
    area: &M2Area,
) -> Option<Vec<CompletionItem>> {
    if text.contains('/') {
        let module_name = text.split('/').next()?;
        let mut files = vec![];
        if let Some(path) = state.lock().get_module_path(module_name) {
            for area in area.path_candidates() {
                let view_path = path.append(&["view", area, "web"]);
                let glob_path = view_path.append(&["**", "*.js"]);
                files.extend(glob::glob(glob_path.to_path_str()).ok()?.map(|file| {
                    let path = file
                        .unwrap_or_default()
                        .relative_to(&view_path)
                        .str_components()
                        .join("/");
                    let path = path.trim_end_matches(".js");
                    String::from(module_name) + "/" + path
                }));
            }
        }
        let workspaces = state.lock().workspace_paths();
        for path in workspaces {
            let view_path = path.append(&["lib", "web"]);
            let glob_path = view_path.append(&["**", "*.js"]);
            files.extend(glob::glob(glob_path.to_path_str()).ok()?.map(|file| {
                let path = file
                    .unwrap_or_default()
                    .relative_to(&view_path)
                    .str_components()
                    .join("/");
                path.trim_end_matches(".js").to_string()
            }));
        }

        files.extend(state.lock().get_component_maps_for_area(area));
        if let Some(lower_area) = area.lower_area() {
            files.extend(state.lock().get_component_maps_for_area(&lower_area));
        }
        Some(string_vec_and_range_to_completion_list(files, range))
    } else {
        let mut modules = vec![];
        modules.extend(state.lock().get_modules());
        modules.extend(state.lock().get_component_maps_for_area(area));
        if let Some(lower_area) = area.lower_area() {
            modules.extend(state.lock().get_component_maps_for_area(&lower_area));
        }
        let workspaces = state.lock().workspace_paths();
        for path in workspaces {
            let view_path = path.append(&["lib", "web"]);
            let glob_path = view_path.append(&["**", "*.js"]);
            modules.extend(glob::glob(glob_path.to_path_str()).ok()?.map(|file| {
                let path = file
                    .unwrap_or_default()
                    .relative_to(&view_path)
                    .str_components()
                    .join("/");
                path.trim_end_matches(".js").to_string()
            }));
        }
        Some(string_vec_and_range_to_completion_list(modules, range))
    }
}

fn string_vec_and_range_to_completion_list(
    mut strings: Vec<String>,
    range: Range,
) -> Vec<CompletionItem> {
    strings.sort_unstable();
    strings.dedup();
    strings
        .iter()
        .map(|label| CompletionItem {
            label: label.clone(),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range,
                new_text: label.clone(),
            })),
            label_details: None,
            kind: Some(CompletionItemKind::FILE),
            detail: None,
            ..CompletionItem::default()
        })
        .collect()
}
