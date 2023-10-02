mod events;

use glob::glob;
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionTextEdit, Range, TextEdit,
};

use crate::{
    indexer::ArcIndexer,
    m2::{self, M2Area, M2Path, M2Uri},
    xml,
};

pub fn get_completion_from_params(
    index: &ArcIndexer,
    params: &CompletionParams,
) -> Option<Vec<CompletionItem>> {
    let path = params
        .text_document_position
        .text_document
        .uri
        .to_path_buf();
    let pos = params.text_document_position.position;
    let content = index.lock().get_file(&path)?.clone();

    if path.is_xml() {
        let at_position = xml::get_current_position_path(&content, pos)?;
        match at_position {
            x if x.match_path("[@template]") => {
                completion_for_template(index, &x.text, &path.get_area())
            }
            x if x.match_path("/config/event[@name]") && path.ends_with("events.xml") => {
                Some(events::get_completion_items())
            }
            x if x.match_path("/config/preference[@for]") && path.ends_with("di.xml") => {
                completion_for_classes(index, &x.text, x.range)
            }
            x if x.match_path("/config/preference[@type]") && path.ends_with("di.xml") => {
                completion_for_classes(index, &x.text, x.range)
            }
            x if x.match_path("[@class]") || x.match_path("[@instance]") => {
                completion_for_classes(index, &x.text, x.range)
            }
            x if x.match_path("/type/arguments/argument") => {
                completion_for_classes(index, &x.text, x.range)
            }
            _ => None,
        }
    } else {
        None
    }
}

fn completion_for_classes(
    index: &ArcIndexer,
    text: &str,
    range: Range,
) -> Option<Vec<CompletionItem>> {
    let text = text.trim_start_matches('\\');
    if text.is_empty() || (m2::is_part_of_class_name(text) && text.matches('\\').count() == 0) {
        Some(completion_for_classes_prefix(index, range))
    } else if text.matches('\\').count() == 1 {
        let mut result = completion_for_classes_prefix(index, range);
        if let Some(classes) = completion_for_classes_full(index, text, range) {
            result.extend(classes);
        }
        Some(result)
    } else if text.matches('\\').count() >= 2 {
        completion_for_classes_full(index, text, range)
    } else {
        None
    }
}

fn completion_for_classes_prefix(index: &ArcIndexer, range: Range) -> Vec<CompletionItem> {
    index
        .lock()
        .get_module_class_prefixes()
        .iter()
        .map(|c| CompletionItem {
            label: c.clone(),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range,
                new_text: c.clone(),
            })),
            label_details: None,
            kind: Some(CompletionItemKind::CLASS),
            detail: None,
            ..CompletionItem::default()
        })
        .collect()
}

fn completion_for_classes_full(
    index: &ArcIndexer,
    text: &str,
    range: Range,
) -> Option<Vec<CompletionItem>> {
    let mut parts = text.split('\\');

    let module_name = format!("{}_{}", parts.next()?, parts.next()?);

    let mut parts = text.split('\\').collect::<Vec<&str>>();
    parts.pop();
    let typed_class_prefix = parts.join("\\");

    let module_class = module_name.replace('_', "\\");
    let module_path = index.lock().get_module_path(&module_name)?;
    let candidates = glob(&module_path.append(&["**", "*.php"]).to_path_string())
        .expect("Failed to read glob pattern");
    let mut result = vec![];
    for p in candidates {
        let path = p.map_or_else(|_| std::path::PathBuf::new(), |p| p);
        let rel_path = path
            .relative_to(&module_path)
            .string_components()
            .join("\\");
        let class_suffix = rel_path.trim_end_matches(".php");
        let class = format!("{}\\{}", &module_class, class_suffix);

        if class.ends_with("\\registration") {
            continue;
        }

        if !class.starts_with(&typed_class_prefix) {
            continue;
        }

        result.push(CompletionItem {
            label: class.clone(),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range,
                new_text: class,
            })),
            label_details: None,
            kind: Some(CompletionItemKind::CLASS),
            detail: None,
            ..CompletionItem::default()
        });
    }
    Some(result)
}

fn completion_for_template(
    index: &ArcIndexer,
    text: &str,
    area: &M2Area,
) -> Option<Vec<CompletionItem>> {
    if text.is_empty() || m2::is_part_of_module_name(text) {
        Some(
            index
                .lock()
                .get_modules()
                .iter()
                .map(|module| CompletionItem {
                    label: module.clone(),
                    label_details: None,
                    kind: Some(CompletionItemKind::MODULE),
                    detail: None,
                    ..CompletionItem::default()
                })
                .collect(),
        )
    } else if text.contains("::") {
        let module_name = text.split("::").next()?;
        let path = index.lock().get_module_path(module_name);
        match path {
            None => None,
            Some(path) => {
                let mut files = vec![];
                for area_string in area.path_candidates() {
                    let view_path = path.append(&["view", &area_string, "templates"]);
                    let glob_path = view_path.append(&["**", "*.phtml"]);
                    files.extend(glob::glob(&glob_path.to_path_string()).ok()?.map(|file| {
                        CompletionItem {
                            label: file
                                .unwrap_or_default()
                                .relative_to(&view_path)
                                .to_path_string(),
                            label_details: None,
                            kind: Some(CompletionItemKind::FILE),
                            detail: None,
                            ..CompletionItem::default()
                        }
                    }));
                }
                files.sort_unstable_by(|a, b| a.label.cmp(&b.label));
                files.dedup();
                Some(files)
            }
        }
    } else {
        None
    }
}
