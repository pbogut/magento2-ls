mod indexer;
mod js;
mod lsp;
mod m2;
mod php;
mod queries;
mod ts;
mod xml;

use std::error::Error;

use anyhow::{Context, Result};
use lsp_server::{Connection, ExtractError, Message, Request, RequestId, Response};
use lsp_types::{
    request::{Completion, GotoDefinition},
    CompletionOptions, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, OneOf, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    WorkDoneProgressOptions,
};

use crate::{indexer::Indexer, m2::M2Uri};

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Note that  we must have our logging only write out to stderr.
    eprintln!("Starting magento2-ls LSP server");

    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let server_capabilities = serde_json::to_value(ServerCapabilities {
        definition_provider: Some(OneOf::Left(true)),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(false),
            trigger_characters: Some(vec![
                ">".to_string(),
                "\"".to_string(),
                "'".to_string(),
                ":".to_string(),
                "\\".to_string(),
                "/".to_string(),
            ]),
            work_done_progress_options: WorkDoneProgressOptions {
                work_done_progress: None,
            },
            all_commit_characters: None,
            completion_item: None,
        }),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL), //TODO change to INCREMENTAL
                will_save: None,
                will_save_wait_until: None,
                // save: Some(SaveOptions::default().into()),
                save: None,
            },
        )),
        ..Default::default()
    })
    .context("Deserializing server capabilities")?;
    let initialization_params = connection.initialize(server_capabilities)?;

    main_loop(&connection, initialization_params)?;
    io_threads.join()?;

    // Shut down gracefully.
    eprintln!("shutting down server");
    Ok(())
}

fn main_loop(
    connection: &Connection,
    init_params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let params: InitializeParams =
        serde_json::from_value(init_params).context("Deserializing initialize params")?;

    let indexer = Indexer::new().into_arc();
    let mut threads = vec![];

    if let Some(uri) = params.root_uri {
        let path = uri.to_file_path().expect("Invalid root path");
        threads.extend(Indexer::update_index(&indexer, &path));
    };

    if let Some(folders) = params.workspace_folders {
        for folder in folders {
            let path = folder.uri.to_file_path().expect("Invalid workspace path");
            threads.extend(Indexer::update_index(&indexer, &path));
        }
    }

    eprintln!("Starting main loop");
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                #[cfg(debug_assertions)]
                eprintln!("request: {:?}", req.method);
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                match req.method.as_str() {
                    "textDocument/completion" => {
                        let (id, params) = cast::<Completion>(req)?;
                        let result = lsp::completion_handler(&indexer, &params);
                        connection.sender.send(get_response_message(id, result))?;
                    }
                    "textDocument/definition" => {
                        let (id, params) = cast::<GotoDefinition>(req)?;
                        let result = lsp::definition_handler(&indexer, &params);
                        connection.sender.send(get_response_message(id, result))?;
                    }
                    _ => {
                        eprintln!("unhandled request: {:?}", req.method);
                    }
                }
            }
            Message::Response(_resp) => {
                #[cfg(debug_assertions)]
                eprintln!("response: {_resp:?}");
            }
            Message::Notification(not) => match not.method.as_str() {
                "textDocument/didOpen" => {
                    let params: DidOpenTextDocumentParams = serde_json::from_value(not.params)
                        .context("Deserializing notification params")?;
                    let path = params.text_document.uri.to_path_buf();
                    indexer.lock().set_file(&path, params.text_document.text);
                    #[cfg(debug_assertions)]
                    eprintln!("textDocument/didOpen: {path:?}");
                }
                "textDocument/didChange" => {
                    let params: DidChangeTextDocumentParams = serde_json::from_value(not.params)
                        .context("Deserializing notification params")?;
                    let path = params.text_document.uri.to_path_buf();
                    indexer
                        .lock()
                        .set_file(&path, &params.content_changes[0].text);
                    #[cfg(debug_assertions)]
                    eprintln!("textDocument/didChange: {path:?}");
                }
                "textDocument/didClose" => {
                    let params: DidCloseTextDocumentParams = serde_json::from_value(not.params)
                        .context("Deserializing notification params")?;
                    let path = params.text_document.uri.to_path_buf();
                    indexer.lock().del_file(&path);
                    #[cfg(debug_assertions)]
                    eprintln!("textDocument/didClose: {path:?}");
                }
                _ => {
                    eprintln!("unhandled notification: {:?}", not.method);
                }
            },
        }
    }

    for thread in threads {
        thread.join().ok();
    }

    Ok(())
}

fn get_response_message<T>(id: RequestId, result: T) -> Message
where
    T: serde::Serialize,
{
    let result = serde_json::to_value(&result).expect("Error serializing response");
    Message::Response(Response {
        id,
        result: Some(result),
        error: None,
    })
}

fn cast<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}
