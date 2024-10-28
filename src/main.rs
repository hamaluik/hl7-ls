use color_eyre::eyre::Context;
use color_eyre::Result;
use lsp_server::{Connection, ExtractError, Message, Request, RequestId};
use lsp_textdocument::TextDocuments;
use lsp_types::notification::{DidChangeTextDocument, DidOpenTextDocument};
use lsp_types::request::{DocumentSymbolRequest, HoverRequest};
use lsp_types::{
    ClientCapabilities, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    HoverProviderCapability, OneOf, PositionEncodingKind, TextDocumentSyncCapability,
    TextDocumentSyncKind,
};
use lsp_types::{InitializeParams, ServerCapabilities};
use std::io::Write;
use utils::build_response;

mod diagnostics;
mod document_symbols;
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
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::INCREMENTAL,
        )),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        document_symbol_provider: Some(OneOf::Right(lsp_types::DocumentSymbolOptions {
            label: Some("HL7 Document".to_string()),
            work_done_progress_options: Default::default(),
        })),
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
    let mut documents = TextDocuments::new();

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
                        let resp = hover::handle_hover_request(params, &documents).map_err(|e| {
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

                let req = match cast_request::<DocumentSymbolRequest>(req) {
                    Ok((id, params)) => {
                        tracing::trace!(id = ?id, "got DocumentSymbol request");
                        let resp =
                            document_symbols::handle_document_symbols_request(params, &documents)
                                .map_err(|e| {
                                    tracing::warn!(
                                        "Failed to handle document symbols request: {e:?}"
                                    );
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
                if documents.listen(not.method.as_str(), &not.params) {
                    if !diagnostics_enabled {
                        continue;
                    }

                    // document was updated, update diagnostics
                    // first, extract the uri
                    let (uri, version) = match not.method.as_str() {
                        <DidOpenTextDocument as lsp_types::notification::Notification>::METHOD => {
                            let params: DidOpenTextDocumentParams = serde_json::from_value(not.params.clone())
                                .expect("Expect receive DidOpenTextDocumentParams");
                            let text_document = params.text_document;
                            (Some(text_document.uri), Some(text_document.version))
                        },
                        <DidChangeTextDocument as lsp_types::notification::Notification>::METHOD => {
                            let params: DidChangeTextDocumentParams = serde_json::from_value(not.params.clone())
                                .expect("Expect receive DidChangeTextDocumentParams");
                            let text_document = params.text_document;
                            (Some(text_document.uri), Some(text_document.version))
                        },
                        _ => (None, None),
                    };

                    let text = uri
                        .as_ref()
                        .and_then(|uri| documents.get_document_content(uri, None));
                    if let Some(text) = text {
                        let errors = match hl7_parser::parse_message_with_lenient_newlines(text) {
                            Ok(message) => validation::validate_message(&message)
                                .into_iter()
                                .map(|e| e.to_diagnostic(text))
                                .collect(),
                            Err(err) => vec![diagnostics::parse_error_to_diagnostic(text, err)],
                        };
                        if errors.is_empty() {
                            diagnostics::clear_diagnostics(&connection, uri.expect("document uri"));
                        } else {
                            diagnostics::publish_parse_error_diagnostics(
                                &connection,
                                uri.expect("document uri"),
                                errors,
                                version,
                            );
                        }
                    } else {
                        diagnostics::clear_diagnostics(&connection, uri.expect("document uri"));
                    }
                } else {
                    tracing::warn!("unhandled notification: {not:?}");
                }
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

