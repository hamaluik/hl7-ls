use chrono::{DateTime, Utc};
use color_eyre::{eyre::ContextCompat, Result};
use hl7_parser::timestamps::TimeStamp;
use lsp_textdocument::TextDocuments;
use lsp_types::{ExecuteCommandParams, Range, TextEdit, Uri, WorkspaceEdit};
use std::collections::HashMap;
use tracing::instrument;

pub const CMD_SET_TO_NOW: &str = "hl7.setTimestampToNow";

#[instrument(level = "debug", skip(params, documents))]
pub fn handle_execute_command_request(
    params: ExecuteCommandParams,
    documents: &TextDocuments,
) -> Result<Option<(&'static str, WorkspaceEdit)>> {
    match params.command.as_str() {
        CMD_SET_TO_NOW => handle_set_to_now_command(params, documents),
        _ => {
            tracing::warn!(command = ?params.command, "Unknown command");
            Ok(None)
        }
    }
}

#[instrument(level = "trace", skip(_documents))]
fn handle_set_to_now_command(
    params: ExecuteCommandParams,
    _documents: &TextDocuments,
) -> Result<Option<(&'static str, WorkspaceEdit)>> {
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
    let mut changes = HashMap::new();
    changes.insert(
        uri,
        vec![TextEdit {
            range,
            new_text: now,
        }],
    );

    Ok(Some((
        "Set timestamp to now",
        WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        },
    )))
}
