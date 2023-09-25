mod js;
mod php;
mod ts;
mod xml;

use std::{collections::HashMap, error::Error, path::PathBuf, time::SystemTime};

use anyhow::{Context, Result};
use lsp_server::{Connection, ExtractError, Message, Request, RequestId, Response};
use lsp_types::{
    request::GotoDefinition, GotoDefinitionResponse, InitializeParams, ServerCapabilities,
};
use lsp_types::{GotoDefinitionParams, Location, OneOf, Range, Url};
use php::{parse_php_file, M2Item, PHPClass};

#[derive(Debug, Clone)]
pub struct Indexer {
    pub php_classes: HashMap<String, PHPClass>,
    pub magento_modules: HashMap<String, PathBuf>,
    pub magento_front_themes: HashMap<String, PathBuf>,
    pub magento_admin_themes: HashMap<String, PathBuf>,
    pub js_maps: HashMap<String, String>,
    pub js_mixins: HashMap<String, String>,
    pub root_path: Option<PathBuf>,
}

impl Indexer {
    pub fn new() -> Self {
        Self {
            php_classes: HashMap::new(),
            magento_modules: HashMap::new(),
            magento_front_themes: HashMap::new(),
            magento_admin_themes: HashMap::new(),
            js_maps: HashMap::new(),
            js_mixins: HashMap::new(),
            root_path: None,
        }
    }
}

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Note that  we must have our logging only write out to stderr.
    eprintln!("Starting magento2-ls LSP server");

    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let server_capabilities = serde_json::to_value(ServerCapabilities {
        definition_provider: Some(OneOf::Left(true)),
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

    let index_start = SystemTime::now();

    eprint!("Preparing index...");

    let root_uri = params.root_uri.context("Root uri is required")?;
    let root_path = root_uri
        .to_file_path()
        .expect("Root uri should be valid file path");
    let mut indexer = Indexer::new();
    indexer.root_path = Some(root_path.clone());

    php::update_index(&mut indexer, &PathBuf::from(&root_path));
    js::update_index(&mut indexer, &PathBuf::from(&root_path));

    index_start
        .elapsed()
        .map_or_else(|_| eprintln!(" done"), |d| eprintln!(" done in {:?}", d));

    eprintln!("Starting main loop");
    for msg in &connection.receiver {
        eprintln!("got msg: {msg:?}");
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                eprintln!("got request: {req:?}");
                match cast::<GotoDefinition>(req) {
                    Ok((id, params)) => {
                        eprintln!("got gotoDefinition request #{id}: {params:?}");
                        let result = Some(GotoDefinitionResponse::Array(
                            get_location_from_params(&mut indexer, params)
                                .map_or(vec![], |loc_list| loc_list),
                        ));

                        let result =
                            serde_json::to_value(&result).context("Error serializing response")?;
                        let resp = Response {
                            id,
                            result: Some(result),
                            error: None,
                        };
                        connection.sender.send(Message::Response(resp))?;
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
                    Err(ExtractError::MethodMismatch(req)) => req,
                };
                // ...
            }
            Message::Response(resp) => {
                eprintln!("got response: {resp:?}");
            }
            Message::Notification(not) => {
                eprintln!("got notification: {not:?}");
            }
        }
    }
    Ok(())
}

fn get_module_path(index: &Indexer, class: &str) -> Option<(PathBuf, Vec<String>)> {
    let mut parts = class.split('\\').collect::<Vec<_>>();
    let mut suffix = vec![];

    while let Some(part) = parts.pop() {
        suffix.push(part.to_string());
        let prefix = parts.join("\\");

        match index.magento_modules.get(&prefix) {
            Some(mod_path) => {
                suffix.reverse();
                return Some((mod_path.clone(), suffix));
            }
            None => continue,
        }
    }

    None
}

fn get_php_class_from_class_name(index: &mut Indexer, class: &str) -> Option<PHPClass> {
    match index.php_classes.get(class) {
        Some(phpclass) => Some(phpclass.clone()),
        None => {
            match get_module_path(index, class) {
                None => None,
                Some((mut file_path, suffix)) => {
                    for part in suffix {
                        file_path.push(part);
                    }
                    file_path.set_extension("php");

                    match file_path.try_exists() {
                        Ok(true) => {
                            let phpclass = parse_php_file(&file_path)?;
                            // update indexer for future use
                            index
                                .php_classes
                                .insert(class.to_string(), phpclass.clone());
                            Some(phpclass)
                        }
                        _ => None,
                    }
                }
            }
        }
    }
}

fn get_location_from_params(
    index: &mut Indexer,
    params: GotoDefinitionParams,
) -> Option<Vec<Location>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    let file_path = uri.to_file_path().expect("Should be valid file path");

    match file_path.extension()?.to_str()?.to_lowercase().as_str() {
        "js" => match js::get_item_from_position(&index, &uri, pos) {
            Some(js::M2Item::ModComponent(_mod_name, file_path, mod_path)) => {
                let mut result = vec![];
                for uri in js::make_web_uris(&mod_path, &PathBuf::from(&file_path)) {
                    result.push(Location {
                        uri,
                        range: Range::default(),
                    });
                }

                Some(result)
            }
            Some(js::M2Item::RelComponent(comp, path)) => {
                let mut path = path.join(comp);
                path.set_extension("js");
                if path.exists() {
                    Some(vec![Location {
                        uri: Url::from_file_path(path).expect("Should be valid url"),
                        range: Range::default(),
                    }])
                } else {
                    None
                }
            }
            Some(js::M2Item::Component(comp)) => {
                let mut p3 = index.root_path.clone()?.join("lib").join("web").join(&comp);
                p3.set_extension("js");
                if p3.exists() {
                    Some(vec![Location {
                        uri: Url::from_file_path(p3).expect("Should be valid url"),
                        range: Range::default(),
                    }])
                } else {
                    None
                }
            }
            None => None,
        },
        "xml" => match xml::get_item_from_position(&uri, pos) {
            Some(M2Item::AdminPhtml(mod_name, template)) => {
                let mut result = vec![];
                let mod_path = index.magento_modules.get(&mod_name);
                if let Some(path) = mod_path {
                    let templ_path = path
                        .join("view")
                        .join("adminhtml")
                        .join("templates")
                        .join(&template);
                    if templ_path.is_file() {
                        result.push(Location {
                            uri: Url::from_file_path(templ_path).expect("Should be valid Url"),
                            range: Range::default(),
                        });
                    }
                };

                for theme_path in index.magento_admin_themes.values() {
                    let templ_path = theme_path.join(&mod_name).join("templates").join(&template);
                    if templ_path.is_file() {
                        result.push(Location {
                            uri: Url::from_file_path(templ_path).expect("Should be valid url"),
                            range: Range::default(),
                        });
                    }
                }

                Some(result)
            }
            Some(M2Item::FrontPhtml(mod_name, template)) => {
                let mut result = vec![];
                let mod_path = index.magento_modules.get(&mod_name);
                if let Some(path) = mod_path {
                    let templ_path = path
                        .join("view")
                        .join("frontend")
                        .join("templates")
                        .join(&template);
                    if templ_path.is_file() {
                        result.push(Location {
                            uri: Url::from_file_path(templ_path).expect("Should be valid Url"),
                            range: Range::default(),
                        });
                    }
                };

                for theme_path in index.magento_front_themes.values() {
                    let templ_path = theme_path.join(&mod_name).join("templates").join(&template);
                    if templ_path.is_file() {
                        result.push(Location {
                            uri: Url::from_file_path(templ_path).expect("Should be valid url"),
                            range: Range::default(),
                        });
                    }
                }

                Some(result)
            }
            Some(M2Item::Class(class)) => {
                let phpclass = get_php_class_from_class_name(index, &class)?;
                index.php_classes.insert(class.clone(), phpclass.clone());
                Some(vec![Location {
                    uri: phpclass.uri.clone(),
                    range: phpclass.range,
                }])
            }
            Some(M2Item::Method(class, method)) => {
                let phpclass = get_php_class_from_class_name(index, &class)?;
                index.php_classes.insert(class.clone(), phpclass.clone());

                Some(vec![Location {
                    uri: phpclass.uri.clone(),
                    range: phpclass
                        .methods
                        .get(&method)
                        .map_or(phpclass.range, |method| method.range),
                }])
            }
            Some(M2Item::Const(class, constant)) => {
                let phpclass = get_php_class_from_class_name(index, &class)?;
                index.php_classes.insert(class.clone(), phpclass.clone());

                Some(vec![Location {
                    uri: phpclass.uri.clone(),
                    range: phpclass
                        .constants
                        .get(&constant)
                        .map_or(phpclass.range, |method| method.range),
                }])
            }
            None => None,
        },
        _ => None,
    }
}

fn cast<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}
