use std::collections::HashMap;

use crate::utils::lsp_range_to_std_range;

use super::CommandResult;
use color_eyre::{eyre::ContextCompat, Result};
use hl7_parser::parse_message_with_lenient_newlines;
use lsp_textdocument::TextDocuments;
use lsp_types::{ExecuteCommandParams, Range, TextEdit, Uri, WorkspaceEdit};
use tracing::instrument;

#[instrument(level = "debug", skip(documents))]
pub fn handle_encode_selection_command(
    params: ExecuteCommandParams,
    documents: &TextDocuments,
) -> Result<Option<CommandResult>> {
    assert_eq!(
        params.arguments.len(),
        2,
        "Expected 2 arguments for encode selection command"
    );

    let uri: Uri = params.arguments[0]
        .as_str()
        .and_then(|s| s.parse().ok())
        .wrap_err("Expected uri as first argument")?;

    let range: Range = params.arguments[1]
        .as_object()
        .and_then(|obj| serde_json::from_value(serde_json::Value::Object(obj.clone())).ok())
        .wrap_err("Expected range as second argument")?;

    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;

    let parse_span = tracing::trace_span!("parse message");
    let _parse_span_guard = parse_span.enter();
    let separators = parse_message_with_lenient_newlines(text)
        .ok()
        .map(|message| message.separators)
        .unwrap_or_default();
    drop(_parse_span_guard);

    let Some(std_range) = lsp_range_to_std_range(text, range) else {
        return Err(color_eyre::eyre::eyre!("Invalid range"));
    };
    let encoded = separators.encode(&text[std_range.clone()]).to_string();

    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range,
            new_text: encoded,
        }],
    );

    Ok(Some(CommandResult::WorkspaceEdit {
        label: "Encode selection",
        edit: WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        },
    }))
}

#[instrument(level = "debug", skip(documents))]
pub fn handle_decode_selection_command(
    params: ExecuteCommandParams,
    documents: &TextDocuments,
) -> Result<Option<CommandResult>> {
    assert_eq!(
        params.arguments.len(),
        2,
        "Expected 2 arguments for decode selection command"
    );

    let uri: Uri = params.arguments[0]
        .as_str()
        .and_then(|s| s.parse().ok())
        .wrap_err("Expected uri as first argument")?;

    let range: Range = params.arguments[1]
        .as_object()
        .and_then(|obj| serde_json::from_value(serde_json::Value::Object(obj.clone())).ok())
        .wrap_err("Expected range as second argument")?;

    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;

    let parse_span = tracing::trace_span!("parse message");
    let _parse_span_guard = parse_span.enter();
    let separators = parse_message_with_lenient_newlines(text)
        .ok()
        .map(|message| message.separators)
        .unwrap_or_default();
    drop(_parse_span_guard);

    let Some(std_range) = lsp_range_to_std_range(text, range) else {
        return Err(color_eyre::eyre::eyre!("Invalid range"));
    };
    let encoded = separators.decode(&text[std_range.clone()]).to_string();

    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range,
            new_text: encoded,
        }],
    );

    Ok(Some(CommandResult::WorkspaceEdit {
        label: "Encode selection",
        edit: WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        },
    }))
}
