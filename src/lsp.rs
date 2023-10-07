mod completion;
mod definition;

use lsp_types::{
    CompletionParams, CompletionResponse, GotoDefinitionParams, GotoDefinitionResponse,
};

use crate::state::ArcState;

use self::{completion::get_completion_from_params, definition::get_location_from_params};
pub fn completion_handler(state: &ArcState, params: &CompletionParams) -> CompletionResponse {
    CompletionResponse::Array(
        get_completion_from_params(state, params).map_or(vec![], |loc_list| loc_list),
    )
}

pub fn definition_handler(
    state: &ArcState,
    params: &GotoDefinitionParams,
) -> GotoDefinitionResponse {
    GotoDefinitionResponse::Array(
        get_location_from_params(state, params).map_or(vec![], |loc_list| loc_list),
    )
}
