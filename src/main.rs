use cli::Cli;
use color_eyre::eyre::Context;
use color_eyre::Result;
use lsp_server::{Connection, ExtractError, Message, Request, RequestId, Response, ResponseError};
use lsp_textdocument::TextDocuments;
use lsp_types::notification::{DidChangeTextDocument, DidOpenTextDocument};
use lsp_types::request::{
    ApplyWorkspaceEdit, CodeActionRequest, CodeLensRequest, Completion, DocumentSymbolRequest,
    ExecuteCommand, HoverRequest, Request as LspRequest,
};
use lsp_types::{
    ApplyWorkspaceEditParams, ClientCapabilities, CodeActionOptions, CodeActionProviderCapability,
    CodeLensOptions, CompletionOptions, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    ExecuteCommandOptions, HoverProviderCapability, OneOf, PositionEncodingKind,
    TextDocumentSyncCapability, TextDocumentSyncKind,
};
use lsp_types::{InitializeParams, ServerCapabilities};
use std::fs::{self};
use std::io::IsTerminal;
use tracing::instrument;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{filter, prelude::*, Registry};
use utils::build_response;

mod cli;
mod code_actions;
mod codelens;
mod commands;
mod completion;
mod diagnostics;
mod document_symbols;
mod hover;
pub mod spec;
pub mod utils;
mod validation;

fn setup_logging(cli: Cli) -> Result<()> {
    let use_colours = match (cli.colour, &cli.command) {
        (clap::ColorChoice::Never, _) => false,
        (clap::ColorChoice::Always, _) => true,
        (_, Some(cli::Commands::LogToFile { .. })) => false,
        (_, Some(cli::Commands::LogToStderr)) => std::io::stderr().is_terminal(),
        (_, None) => std::io::stderr().is_terminal(),
    };

    color_eyre::config::HookBuilder::new()
        .theme(if use_colours {
            color_eyre::config::Theme::dark()
        } else {
            color_eyre::config::Theme::new()
        })
        .install()
        .expect("Failed to install `color_eyre`");

    let log_level = match cli.verbose {
        0 => LevelFilter::INFO,
        1 => LevelFilter::DEBUG,
        _ => LevelFilter::TRACE,
    };

    let log_file = match cli.command {
        Some(cli::Commands::LogToFile { ref log_file }) => Some(log_file),
        _ => None,
    };

    let logs_filter = move |metadata: &tracing::Metadata<'_>| {
        metadata.target().starts_with("hl7_ls") && *metadata.level() <= log_level
    };

    let stderr_log = if log_file.is_none() {
        Some(
            tracing_subscriber::fmt::layer()
                // .pretty()
                .with_ansi(use_colours)
                .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
                .with_target(false)
                .with_level(true)
                .with_writer(std::io::stderr)
                .with_filter(filter::filter_fn(logs_filter)),
        )
    } else {
        None
    };

    let file_log = if let Some(log_file) = log_file {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .wrap_err_with(|| format!("Failed to open log file: {log_file:?}"))?;
        Some(
            tracing_subscriber::fmt::layer()
                // .json()
                // .pretty()
                .with_ansi(use_colours)
                .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
                .with_target(false)
                .with_level(true)
                .with_writer(file)
                .with_filter(filter::filter_fn(logs_filter)),
        )
    } else {
        None
    };

    Registry::default().with(stderr_log).with(file_log).init();
    Ok(())
}

fn main() -> Result<()> {
    let cli = cli::cli();
    setup_logging(cli).wrap_err_with(|| "Failed to setup logging")?;

    tracing::info!("Starting HL7 Language Server");
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
            "Client does not support UTF-8 position encoding, unicode stuff will probably be broken"
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
        completion_provider: Some(CompletionOptions {
            ..Default::default()
        }),
        code_lens_provider: Some(CodeLensOptions {
            resolve_provider: Some(false),
        }),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![lsp_types::CodeActionKind::QUICKFIX]),
            ..Default::default()
        })),
        execute_command_provider: Some(ExecuteCommandOptions {
            commands: vec![
                commands::CMD_SET_TO_NOW.to_string(),
                commands::CMD_SEND_MESSAGE.to_string(),
            ],
            ..Default::default()
        }),
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
    tracing::info!("Shutting down");
    Ok(())
}

#[instrument(level = "debug", skip(connection, client_capabilities))]
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
                let request_span =
                    tracing::debug_span!("request", method = ?req.method, id = ?req.id);
                let _request_span_guard = request_span.enter();

                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }

                let req = match cast_request::<HoverRequest>(req) {
                    Ok((id, params)) => {
                        tracing::debug!("got Hover request");
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
                        tracing::debug!("got DocumentSymbol request");
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

                let req = match cast_request::<Completion>(req) {
                    Ok((id, params)) => {
                        tracing::debug!("got Completion request");
                        let resp = completion::handle_completion_request(params, &documents)
                            .map_err(|e| {
                                tracing::warn!("Failed to handle completion request: {e:?}");
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

                let req = match cast_request::<CodeActionRequest>(req) {
                    Ok((id, params)) => {
                        tracing::debug!("got CodeAction request");
                        let resp = code_actions::handle_code_actions_request(params, &documents)
                            .map_err(|e| {
                                tracing::warn!("Failed to handle code action request: {e:?}");
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

                let req = match cast_request::<CodeLensRequest>(req) {
                    Ok((id, params)) => {
                        tracing::trace!(id = ?id, "got CodeLens request");
                        let resp =
                            codelens::handle_codelens_request(params, &documents).map_err(|e| {
                                tracing::warn!("Failed to handle codelens request: {e:?}");
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

                let req = match cast_request::<ExecuteCommand>(req) {
                    Ok((id, params)) => {
                        tracing::debug!("got ExecuteCommand request");
                        let result = commands::handle_execute_command_request(params, &documents)
                            .map_err(|e| {
                                tracing::warn!("Failed to handle execute command request: {e:?}");
                                e
                            });

                        let (edit, resp) = match result {
                            Ok(Some(command_result)) => match command_result {
                                commands::CommandResult::WorkspaceEdit { label, edit } => (
                                    Some((label, edit)),
                                    Response {
                                        id,
                                        result: Some(serde_json::Value::Bool(true)),
                                        error: None,
                                    },
                                ),
                                commands::CommandResult::SentMessage { response } => (
                                    None,
                                    Response {
                                        id,
                                        result: Some(serde_json::Value::String(response)),
                                        error: None,
                                    },
                                ),
                            },
                            Ok(None) => (
                                None,
                                Response {
                                    id,
                                    result: Some(serde_json::Value::Null),
                                    error: Some(ResponseError {
                                        code: lsp_server::ErrorCode::RequestFailed as i32,
                                        message: "Unknown command".to_string(),
                                        data: None,
                                    }),
                                },
                            ),
                            Err(error) => (
                                None,
                                Response {
                                    id,
                                    result: None,
                                    error: Some(ResponseError {
                                        code: lsp_server::ErrorCode::InternalError as i32,
                                        message: format!("{error:#}"),
                                        data: None,
                                    }),
                                },
                            ),
                        };
                        connection
                            .sender
                            .send(Message::Response(resp))
                            .expect("can send response");

                        if let Some((label, edit)) = edit {
                            let apply_edit_span = tracing::debug_span!("apply edit");
                            let _apply_edit_span_guard = apply_edit_span.enter();
                            let apply_edit_params = ApplyWorkspaceEditParams {
                                label: Some(label.to_string()),
                                edit,
                            };
                            let request_id: i32 = rand::random();
                            tracing::trace!(?apply_edit_params, ?request_id, "sending apply edit");
                            let apply_edit_req = Request {
                                id: request_id.into(),
                                method: ApplyWorkspaceEdit::METHOD.to_string(),
                                params: serde_json::to_value(apply_edit_params).unwrap(),
                            };
                            connection
                                .sender
                                .send(Message::Request(apply_edit_req))
                                .expect("can send request");
                        }

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
                let notification_span = tracing::debug_span!("notification", method = ?not.method);
                let _notification_span_guard = notification_span.enter();

                if documents.listen(not.method.as_str(), &not.params) {
                    if !diagnostics_enabled {
                        continue;
                    }

                    let diagnostics_span = tracing::debug_span!("diagnostics");
                    let _diagnostics_span_guard = diagnostics_span.enter();

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
                        let parse_and_validate_span = tracing::debug_span!("parse and validate");
                        let _parse_and_validate_span_guard = parse_and_validate_span.enter();
                        let errors = match hl7_parser::parse_message_with_lenient_newlines(text) {
                            Ok(message) => validation::validate_message(&message)
                                .into_iter()
                                .map(|e| e.into_diagnostic(text))
                                .collect(),
                            Err(err) => vec![diagnostics::parse_error_to_diagnostic(text, err)],
                        };
                        drop(_parse_and_validate_span_guard);
                        let publish_diagnostics_span = tracing::debug_span!("publish diagnostics");
                        let _publish_diagnostics_span_guard = publish_diagnostics_span.enter();
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
