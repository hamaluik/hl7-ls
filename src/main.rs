use cli::Cli;
use color_eyre::eyre::Context;
use color_eyre::Result;
use crossbeam_channel::select;
use lsp_server::{Connection, ExtractError, Message, Request, RequestId, Response, ResponseError};
use lsp_textdocument::TextDocuments;
use lsp_types::notification::{
    self, DidChangeTextDocument, DidOpenTextDocument, LogMessage, Notification,
};
use lsp_types::request::{
    ApplyWorkspaceEdit, CodeActionRequest, Completion, DocumentSymbolRequest, ExecuteCommand,
    HoverRequest, Request as LspRequest, SelectionRangeRequest,
};
use lsp_types::{
    ApplyWorkspaceEditParams, ClientCapabilities, CodeActionOptions, CodeActionProviderCapability,
    CompletionOptions, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    ExecuteCommandOptions, HoverProviderCapability, LogMessageParams, MessageType, OneOf,
    PositionEncodingKind, TextDocumentSyncCapability, TextDocumentSyncKind, Uri, WorkspaceFolder,
};
use lsp_types::{InitializeParams, ServerCapabilities};
use std::fs::{self};
use std::io::IsTerminal;
use std::ops::Deref;
use tracing::instrument;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{filter, prelude::*, Registry};
use utils::build_response;
use workspace::Workspace;

mod cli;
mod code_actions;
mod commands;
mod completion;
mod diagnostics;
mod document_symbols;
mod hover;
mod selection_range;
pub mod spec;
pub mod utils;
mod validation;
mod workspace;

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

struct Opts {
    vscode: bool,
    disable_std_table_validations: bool,
}

impl From<&Cli> for Opts {
    fn from(value: &Cli) -> Self {
        Self {
            vscode: value.vscode,
            disable_std_table_validations: value.disable_std_table_validations,
        }
    }
}

fn main() -> Result<()> {
    let cli = cli::cli();
    let opts = (&cli).into();
    setup_logging(cli).wrap_err_with(|| "Failed to setup logging")?;

    let initial_span = tracing::info_span!("initialise");
    let _initial_span_guard = initial_span.enter();
    tracing::info!("Starting HL7 Language Server");
    let (connection, io_threads) = Connection::stdio();

    let (id, params) = connection.initialize_start()?;
    let init_params: InitializeParams = serde_json::from_value(params).unwrap();
    tracing::info!(client_info = ?init_params.client_info, "client connected");
    tracing::debug!(?init_params.workspace_folders, "workspace folders");
    let client_capabilities = init_params.capabilities;
    let workspace_folders = init_params.workspace_folders;

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
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![lsp_types::CodeActionKind::QUICKFIX]),
            ..Default::default()
        })),
        execute_command_provider: Some(ExecuteCommandOptions {
            commands: vec![
                commands::CMD_SET_TO_NOW.to_string(),
                commands::CMD_SEND_MESSAGE.to_string(),
                commands::CMD_GENERATE_CONTROL_ID.to_string(),
            ],
            ..Default::default()
        }),
        selection_range_provider: Some(lsp_types::SelectionRangeProviderCapability::Simple(true)),
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
    drop(_initial_span_guard);

    main_loop(connection, client_capabilities, workspace_folders, opts)?;
    io_threads.join()?;

    // Shut down gracefully.
    tracing::info!("Shutting down\n");
    Ok(())
}

fn send_log_message<S: ToString>(
    connection: &Connection,
    message_type: MessageType,
    message: S,
) -> Result<()> {
    connection
        .sender
        .send(Message::Notification(lsp_server::Notification::new(
            LogMessage::METHOD.to_string(),
            LogMessageParams {
                typ: message_type,
                message: message.to_string(),
            },
        )))
        .wrap_err_with(|| "Failed to send log message")
}

#[instrument(
    level = "debug",
    skip(connection, client_capabilities, workspace_folders, opts)
)]
fn main_loop(
    connection: Connection,
    client_capabilities: ClientCapabilities,
    workspace_folders: Option<Vec<WorkspaceFolder>>,
    opts: Opts,
) -> Result<()> {
    let mut documents = TextDocuments::new();

    let diagnostics_enabled = client_capabilities
        .text_document
        .as_ref()
        .map(|tdc| tdc.publish_diagnostics.is_some())
        .unwrap_or(false);
    tracing::debug!("diagnostics enabled: {diagnostics_enabled}");

    let load_custom_validators_span = tracing::debug_span!("load_custom_validators");
    let _load_custom_validators_span_guard = load_custom_validators_span.enter();
    let workspace = workspace_folders
        .map(Workspace::new)
        .transpose()
        .wrap_err_with(|| "Failed to load custom validators")?;
    if workspace.is_some() {
        tracing::info!("Custom validators loaded");
        send_log_message(&connection, MessageType::INFO, "Custom validators loaded")
            .wrap_err_with(|| "Failed to send log message")?;
    } else {
        tracing::info!("No custom validators found");
    }
    drop(_load_custom_validators_span_guard);

    tracing::debug!("starting main loop");
    if let Some(workspace) = workspace {
        loop {
            select! {
                recv(&connection.receiver) -> msg => {
                    let msg = msg.wrap_err_with(|| "Failed to receive message")?;
                    handle_msg(msg, &connection, &mut documents, &opts, Some(&workspace), diagnostics_enabled)
                        .wrap_err_with(|| "Failed to handle message")?;
                }
                recv(workspace._custom_spec_changes) -> _ => {
                    for (document_uri, document) in documents.documents() {
                        if let Err(e) = handle_diagnostics(&connection, document_uri, Some(document.version()), &documents, Some(&workspace), &opts) {
                            tracing::error!("Failed to handle diagnostics: {e:?}");
                        }
                    }
                }
            }
        }
    } else {
        for msg in &connection.receiver {
            handle_msg(
                msg,
                &connection,
                &mut documents,
                &opts,
                workspace.as_ref(),
                diagnostics_enabled,
            )
            .wrap_err_with(|| "Failed to handle message")?;
        }
    }

    Ok(())
}

fn handle_msg(
    msg: Message,
    connection: &Connection,
    documents: &mut TextDocuments,
    opts: &Opts,
    workspace: Option<&Workspace>,
    diagnostics_enabled: bool,
) -> Result<()> {
    match msg {
        Message::Request(req) => {
            let request_span = tracing::debug_span!("request", method = ?req.method, id = ?req.id);
            let _request_span_guard = request_span.enter();

            if connection.handle_shutdown(&req)? {
                return Ok(());
            }

            if let Some(req) = handle_hover_req(req, &documents, workspace, &opts, &connection)
                .and_then(|req| handle_document_symbols_req(req, &documents, &connection))
                .and_then(|req| handle_completion_request(req, &documents, &connection))
                .and_then(|req| handle_code_action_request(req, &documents, &connection))
                .and_then(|req| handle_command_request(req, &documents, &connection))
                .and_then(|req| handle_selection_range_req(req, &documents, &connection))
            {
                tracing::warn!("unhandled request: {req:?}");
            }
        }
        Message::Response(resp) => {
            tracing::warn!(response = ?resp, "got response from server??");
        }
        Message::Notification(not) => {
            let notification_span = tracing::debug_span!("notification", method = ?not.method);
            let _notification_span_guard = notification_span.enter();

            if documents.listen(not.method.as_str(), &not.params) {
                if !diagnostics_enabled {
                    return Ok(());
                }

                let diagnostics_span = tracing::debug_span!("diagnostics");
                let _diagnostics_span_guard = diagnostics_span.enter();

                // document was updated, update diagnostics
                // first, extract the uri
                let (uri, version) = match not.method.as_str() {
                    <DidOpenTextDocument as notification::Notification>::METHOD => {
                        let params: DidOpenTextDocumentParams =
                            serde_json::from_value(not.params.clone())
                                .expect("Expect receive DidOpenTextDocumentParams");
                        let text_document = params.text_document;
                        (Some(text_document.uri), Some(text_document.version))
                    }
                    <DidChangeTextDocument as notification::Notification>::METHOD => {
                        let params: DidChangeTextDocumentParams =
                            serde_json::from_value(not.params.clone())
                                .expect("Expect receive DidChangeTextDocumentParams");
                        let text_document = params.text_document;
                        (Some(text_document.uri), Some(text_document.version))
                    }
                    _ => (None, None),
                };

                if let Some(uri) = uri {
                    if let Err(e) =
                        handle_diagnostics(&connection, &uri, version, &documents, workspace, &opts)
                    {
                        tracing::error!("Failed to handle diagnostics: {e:?}");
                    }
                }
            } else {
                tracing::warn!("unhandled notification: {not:?}");
            }
        }
    }
    Ok(())
}

#[instrument(level = "debug", skip(connection, documents, workspace, opts))]
fn handle_diagnostics(
    connection: &Connection,
    uri: &Uri,
    version: Option<i32>,
    documents: &TextDocuments,
    workspace: Option<&Workspace>,
    opts: &Opts,
) -> Result<()> {
    let text = documents.get_document_content(uri, None);
    if let Some(text) = text {
        let parse_and_validate_span = tracing::debug_span!("parse and validate");
        let _parse_and_validate_span_guard = parse_and_validate_span.enter();
        let errors = match hl7_parser::parse_message_with_lenient_newlines(text) {
            Ok(message) => validation::validate_message(
                uri,
                &message,
                &workspace.as_ref().map(|w| w.specs.deref()),
                opts,
            )
            .into_iter()
            .map(|e| e.into_diagnostic(text))
            .collect(),
            Err(err) => vec![diagnostics::parse_error_to_diagnostic(text, err)],
        };
        drop(_parse_and_validate_span_guard);
        let publish_diagnostics_span = tracing::debug_span!("publish diagnostics");
        let _publish_diagnostics_span_guard = publish_diagnostics_span.enter();
        if errors.is_empty() {
            diagnostics::clear_diagnostics(connection, uri.clone());
        } else {
            diagnostics::publish_parse_error_diagnostics(connection, uri.clone(), errors, version);
        }
    } else {
        diagnostics::clear_diagnostics(connection, uri.clone());
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

fn handle_hover_req(
    req: Request,
    documents: &TextDocuments,
    workspace: Option<&Workspace>,
    opts: &Opts,
    connection: &Connection,
) -> Option<Request> {
    match cast_request::<HoverRequest>(req) {
        Ok((id, params)) => {
            tracing::debug!("got Hover request");
            let resp = hover::handle_hover_request(
                params,
                documents,
                workspace.as_ref().map(|w| &*w.specs),
                opts,
            )
            .map_err(|e| {
                tracing::warn!("Failed to handle hover request: {e:?}");
                e
            });
            let resp = build_response(id, resp);
            connection
                .sender
                .send(Message::Response(resp))
                .expect("can send response");
            None
        }
        Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
        Err(ExtractError::MethodMismatch(req)) => Some(req),
    }
}

fn handle_document_symbols_req(
    req: Request,
    documents: &TextDocuments,
    connection: &Connection,
) -> Option<Request> {
    match cast_request::<DocumentSymbolRequest>(req) {
        Ok((id, params)) => {
            tracing::debug!("got DocumentSymbol request");
            let resp = document_symbols::handle_document_symbols_request(params, documents)
                .map_err(|e| {
                    tracing::warn!("Failed to handle document symbols request: {e:?}");
                    e
                });
            let resp = build_response(id, resp);
            connection
                .sender
                .send(Message::Response(resp))
                .expect("can send response");
            None
        }
        Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
        Err(ExtractError::MethodMismatch(req)) => Some(req),
    }
}

fn handle_completion_request(
    req: Request,
    documents: &TextDocuments,
    connection: &Connection,
) -> Option<Request> {
    match cast_request::<Completion>(req) {
        Ok((id, params)) => {
            tracing::debug!("got Completion request");
            let resp = completion::handle_completion_request(params, documents).map_err(|e| {
                tracing::warn!("Failed to handle completion request: {e:?}");
                e
            });
            let resp = build_response(id, resp);
            connection
                .sender
                .send(Message::Response(resp))
                .expect("can send response");
            None
        }
        Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
        Err(ExtractError::MethodMismatch(req)) => Some(req),
    }
}

fn handle_code_action_request(
    req: Request,
    documents: &TextDocuments,
    connection: &Connection,
) -> Option<Request> {
    match cast_request::<CodeActionRequest>(req) {
        Ok((id, params)) => {
            tracing::debug!("got CodeAction request");
            let resp = code_actions::handle_code_actions_request(params, documents).map_err(|e| {
                tracing::warn!("Failed to handle code action request: {e:?}");
                e
            });
            let resp = build_response(id, resp);
            connection
                .sender
                .send(Message::Response(resp))
                .expect("can send response");
            None
        }
        Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
        Err(ExtractError::MethodMismatch(req)) => Some(req),
    }
}

fn handle_command_request(
    req: Request,
    documents: &TextDocuments,
    connection: &Connection,
) -> Option<Request> {
    match cast_request::<ExecuteCommand>(req) {
        Ok((id, params)) => {
            tracing::debug!("got ExecuteCommand request");
            let result = commands::handle_execute_command_request(params, documents).map_err(|e| {
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

            None
        }
        Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
        Err(ExtractError::MethodMismatch(req)) => Some(req),
    }
}

fn handle_selection_range_req(
    req: Request,
    documents: &TextDocuments,
    connection: &Connection,
) -> Option<Request> {
    match cast_request::<SelectionRangeRequest>(req) {
        Ok((id, params)) => {
            tracing::debug!("got SelectionRange request");
            let resp =
                selection_range::handle_selection_range_request(params, documents).map_err(|e| {
                    tracing::warn!("Failed to handle code action request: {e:?}");
                    e
                });
            let resp = build_response(id, resp);
            connection
                .sender
                .send(Message::Response(resp))
                .expect("can send response");
            None
        }
        Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
        Err(ExtractError::MethodMismatch(req)) => Some(req),
    }
}
