mod completion;
mod definition;

use lsp_types::{
    CompletionParams, CompletionResponse, GotoDefinitionParams, GotoDefinitionResponse,
};

use crate::indexer::ArcIndexer;

use self::{completion::get_completion_from_params, definition::get_location_from_params};
pub fn completion_handler(indexer: &ArcIndexer, params: &CompletionParams) -> CompletionResponse {
    CompletionResponse::Array(
        get_completion_from_params(indexer, params).map_or(vec![], |loc_list| loc_list),
    )
}

pub fn definition_handler(
    indexer: &ArcIndexer,
    params: &GotoDefinitionParams,
) -> GotoDefinitionResponse {
    GotoDefinitionResponse::Array(
        get_location_from_params(indexer, params).map_or(vec![], |loc_list| loc_list),
    )
}
