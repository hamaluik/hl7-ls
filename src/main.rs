use color_eyre::eyre::Context;
use color_eyre::Result;
use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId};
use lsp_types::notification::{DidChangeTextDocument, DidOpenTextDocument};
use lsp_types::request::HoverRequest;
use lsp_types::{
    ClientCapabilities, HoverProviderCapability, PositionEncodingKind, TextDocumentSyncCapability,
    TextDocumentSyncKind,
};
use lsp_types::{InitializeParams, ServerCapabilities};
use std::io::Write;
use utils::build_response;

mod diagnostics;
mod docstore;
mod hover;
pub mod spec;
pub mod utils;
pub mod validation;

fn main() -> Result<()> {
    color_eyre::install()?;

    let log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("hl7_ls.log");
    let mut log_file = std::fs::File::create(&log_path)
        .wrap_err_with(|| format!("Failed to create log file {}", log_path.display()))?;
    eprintln!("Logging to {}", log_path.display());
    writeln!(log_file, "---")?;
    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(log_file)
        .init();

    tracing::info!("starting generic LSP server");
    let (connection, io_threads) = Connection::stdio();

    let (id, params) = connection.initialize_start()?;
    let init_params: InitializeParams = serde_json::from_value(params).unwrap();
    tracing::info!(client_info = ?init_params.client_info, "client connected");
    let client_capabilities: ClientCapabilities = init_params.capabilities;

    let client_supports_utf8_positions = client_capabilities
        .general
        .as_ref()
        .and_then(|g| g.position_encodings.as_ref())
        .map(|p| p.contains(&PositionEncodingKind::UTF8))
        .unwrap_or(false);
    let encoding = if client_supports_utf8_positions {
        PositionEncodingKind::UTF8
    } else {
        tracing::warn!(
            "Client does not support UTF-8 position encoding, unicode stuff will be broken"
        );
        PositionEncodingKind::UTF16
    };

    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        position_encoding: Some(encoding),
        // TODO: implement incremental sync
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        // document_symbol_provider: Some(OneOf::Right(lsp_types::DocumentSymbolOptions {
        //     label: Some("HL7 Document".to_string()),
        //     work_done_progress_options: Default::default(),
        // })),
        ..Default::default()
    })
    .expect("can to serialize server capabilities");

    let initialize_data = serde_json::json!({
        "capabilities": server_capabilities,
        "serverInfo": {
            "name": "hl7-ls",
            "version": env!("CARGO_PKG_VERSION")
        }
    });

    connection
        .initialize_finish(id, initialize_data)
        .wrap_err_with(|| "Failed to finish LSP initialisation")?;

    main_loop(connection, client_capabilities)?;
    io_threads.join()?;

    // Shut down gracefully.
    tracing::info!("shutting down server");
    Ok(())
}

fn main_loop(connection: Connection, client_capabilities: ClientCapabilities) -> Result<()> {
    let mut doc_store = docstore::DocStore::default();

    let diagnostics_enabled = client_capabilities
        .text_document
        .as_ref()
        .map(|tdc| tdc.publish_diagnostics.is_some())
        .unwrap_or(false);

    tracing::debug!("starting main loop");
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }

                let req = match cast_request::<HoverRequest>(req) {
                    Ok((id, params)) => {
                        tracing::trace!(id = ?id, "got Hover request");
                        let resp = hover::handle_hover_request(params, &doc_store).map_err(|e| {
                            tracing::warn!("Failed to handle hover request: {e:?}");
                            e
                        });
                        let resp = build_response(id, resp);
                        connection
                            .sender
                            .send(Message::Response(resp))
                            .expect("can send response");
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
                    Err(ExtractError::MethodMismatch(req)) => req,
                };

                tracing::warn!("unhandled request: {req:?}");
            }
            Message::Response(resp) => {
                tracing::warn!(response = ?resp, "got response from server??");
            }
            Message::Notification(not) => {
                let not = match cast_notification::<DidOpenTextDocument>(not) {
                    Ok(params) => {
                        let errors = doc_store
                            .update(params.text_document.uri.clone(), params.text_document.text);

                        if diagnostics_enabled {
                            if errors.is_empty() {
                                diagnostics::clear_diagnostics(
                                    &connection,
                                    params.text_document.uri,
                                );
                            } else {
                                diagnostics::publish_parse_error_diagnostics(
                                    &connection,
                                    params.text_document.uri,
                                    errors,
                                    params.text_document.version,
                                );
                            }
                        }
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
                    Err(ExtractError::MethodMismatch(not)) => not,
                };

                let not = match cast_notification::<DidChangeTextDocument>(not) {
                    Ok(params) => {
                        if params.content_changes.len() != 1 {
                            panic!(
                                "expected exactly one content change, got {len}",
                                len = params.content_changes.len()
                            );
                        }
                        // TODO: handle multiple content changes
                        let errors = doc_store.update(
                            params.text_document.uri.clone(),
                            params.content_changes[0].text.clone(),
                        );

                        if diagnostics_enabled {
                            if errors.is_empty() {
                                diagnostics::clear_diagnostics(
                                    &connection,
                                    params.text_document.uri,
                                );
                            } else {
                                diagnostics::publish_parse_error_diagnostics(
                                    &connection,
                                    params.text_document.uri,
                                    errors,
                                    params.text_document.version,
                                );
                            }
                        }
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
                    Err(ExtractError::MethodMismatch(not)) => not,
                };

                tracing::warn!("unhandled notification: {not:?}");
            }
        }
    }
    Ok(())
}

fn cast_request<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}

fn cast_notification<N>(not: Notification) -> Result<N::Params, ExtractError<Notification>>
where
    N: lsp_types::notification::Notification,
    N::Params: serde::de::DeserializeOwned,
{
    not.extract(N::METHOD)
}
