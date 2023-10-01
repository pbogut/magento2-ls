mod events;

use lsp_types::{CompletionItem, CompletionItemKind, CompletionParams};

use crate::{
    indexer::ArcIndexer,
    m2::{self, M2Area, M2Path, M2Uri},
    xml,
};

pub fn get_completion_from_params(
    index: &ArcIndexer,
    params: &CompletionParams,
) -> Option<Vec<CompletionItem>> {
    let uri = params
        .text_document_position
        .text_document
        .uri
        .to_path_buf();
    let pos = params.text_document_position.position;
    let content = index.lock().get_file(&uri)?.clone();

    if uri.is_xml() {
        let edit_path = xml::get_current_position_path(&content, pos)?;

        match edit_path.path {
            p if p.ends_with("[@template]") => {
                completion_for_template(index, &edit_path.text, &uri.get_area())
            }
            p if p.ends_with("/event/observer[@name]") => Some(events::get_completion_items()),
            _ => None,
        }
    } else {
        None
    }
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
