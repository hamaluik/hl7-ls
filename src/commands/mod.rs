use chrono::{DateTime, Utc};
use color_eyre::{
    eyre::{Context, ContextCompat},
    Result,
};
use hl7_parser::{parse_message_with_lenient_newlines, timestamps::TimeStamp};
use lsp_textdocument::TextDocuments;
use lsp_types::{ExecuteCommandParams, Range, TextEdit, Uri, WorkspaceEdit};
use std::collections::HashMap;
use tracing::instrument;

use crate::utils::std_range_to_lsp_range;

mod send_message;

pub const CMD_SET_TO_NOW: &str = "hl7.setTimestampToNow";
pub const CMD_SEND_MESSAGE: &str = "hl7.sendMessage";
pub const CMD_GENERATE_CONTROL_ID: &str = "hl7.generateControlId";

pub enum CommandResult {
    WorkspaceEdit {
        label: &'static str,
        edit: WorkspaceEdit,
    },
    SentMessage {
        response: String,
    },
}

#[instrument(level = "debug", skip(params, documents))]
pub fn handle_execute_command_request(
    params: ExecuteCommandParams,
    documents: &TextDocuments,
) -> Result<Option<CommandResult>> {
    match params.command.as_str() {
        CMD_SET_TO_NOW => handle_set_to_now_command(params, documents),
        CMD_SEND_MESSAGE => handle_send_message_command(params, documents),
        CMD_GENERATE_CONTROL_ID => handle_generate_control_id_command(params, documents),
        _ => {
            tracing::warn!(command = ?params.command, args = ?params.arguments, "Unknown command");
            Ok(None)
        }
    }
}

#[instrument(level = "trace", skip(_documents))]
fn handle_set_to_now_command(
    params: ExecuteCommandParams,
    _documents: &TextDocuments,
) -> Result<Option<CommandResult>> {
    assert_eq!(
        params.arguments.len(),
        2,
        "Expected 2 arguments for set to now command"
    );

    let uri: Uri = params.arguments[0]
        .as_str()
        .and_then(|s| s.parse().ok())
        .wrap_err("Expected uri as first argument")?;

    let range: Range = params.arguments[1]
        .as_object()
        .and_then(|obj| serde_json::from_value(serde_json::Value::Object(obj.clone())).ok())
        .wrap_err("Expected range as second argument")?;

    let now: DateTime<Utc> = Utc::now();
    let now: TimeStamp = now.into();
    let now = now.to_string();

    tracing::debug!(?uri, ?range, ?now, "Setting timestamp to now");
    #[allow(clippy::mutable_key_type)]
    let mut changes = HashMap::new();
    changes.insert(
        uri,
        vec![TextEdit {
            range,
            new_text: now,
        }],
    );

    Ok(Some(CommandResult::WorkspaceEdit {
        label: "Set timestamp to now",
        edit: WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        },
    }))
}

#[instrument(level = "debug", skip(documents))]
fn handle_send_message_command(
    params: ExecuteCommandParams,
    documents: &TextDocuments,
) -> Result<Option<CommandResult>> {
    if params.arguments.len() < 3 || params.arguments.len() > 4 {
        return Err(color_eyre::eyre::eyre!(
            "Expected 3 or 4 arguments for send message command"
        ));
    }

    let uri: Uri = params.arguments[0]
        .as_str()
        .and_then(|s| s.parse().ok())
        .wrap_err("Expected uri as first argument")?;

    let hostname = params.arguments[1]
        .as_str()
        .wrap_err("Expected hostname as second argument")?;

    let port = params.arguments[2]
        .as_u64()
        .wrap_err("Expected port as third argument")?;

    let timeout = params
        .arguments
        .get(3)
        .and_then(|v| v.as_f64())
        .unwrap_or(5.0);

    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;

    let parse_span = tracing::trace_span!("parse message");
    let _parse_span_guard = parse_span.enter();
    let _message = parse_message_with_lenient_newlines(text)
        .wrap_err_with(|| "Failed to parse HL7 message")?;
    drop(_parse_span_guard);

    tracing::trace!(?uri, ?hostname, ?port, "Sending message");
    let response = send_message::send_message(hostname, port as u16, text, timeout)
        .wrap_err("Failed to send message")?;
    tracing::trace!(?response, "Received response");

    Ok(Some(CommandResult::SentMessage { response }))
}

#[instrument(level = "debug", skip(documents))]
fn handle_generate_control_id_command(
    params: ExecuteCommandParams,
    documents: &TextDocuments,
) -> Result<Option<CommandResult>> {
    assert_eq!(
        params.arguments.len(),
        1,
        "Expected 1 argument for generate control id command"
    );

    let uri: Uri = params.arguments[0]
        .as_str()
        .and_then(|s| s.parse().ok())
        .wrap_err("Expected uri as first argument")?;

    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;

    let parse_span = tracing::trace_span!("parse message");
    let _parse_span_guard = parse_span.enter();
    let message = parse_message_with_lenient_newlines(text)
        .wrap_err_with(|| "Failed to parse HL7 message")?;
    drop(_parse_span_guard);

    let changes = message.query("MSH.10").map(|existing_control_id| {
        use rand::distributions::{Alphanumeric, DistString};
        let new_control_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 20);

        let range = existing_control_id.range();
        #[allow(clippy::mutable_key_type)]
        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
        changes.insert(
            uri.clone(),
            vec![TextEdit {
                range: std_range_to_lsp_range(message.raw_value(), range),
                new_text: new_control_id,
            }],
        );
        changes
    });

    Ok(changes.map(|changes| CommandResult::WorkspaceEdit {
        label: "Generate new control ID",
        edit: WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        },
    }))
}
