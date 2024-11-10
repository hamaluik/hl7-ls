use super::CommandResult;
use chrono::{DateTime, Utc};
use color_eyre::{eyre::ContextCompat, Result};
use hl7_parser::datetime::TimeStamp;
use lsp_textdocument::TextDocuments;
use lsp_types::{ExecuteCommandParams, Range, TextEdit, Uri, WorkspaceEdit};
use std::collections::HashMap;
use tracing::instrument;

#[instrument(level = "trace", skip(_documents))]
pub fn handle_set_to_now_command(
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
