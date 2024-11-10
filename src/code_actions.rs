use crate::{
    commands::{
        CMD_DECODE_SELECTION, CMD_ENCODE_SELECTION, CMD_GENERATE_CONTROL_ID, CMD_SET_TO_NOW,
    },
    spec,
    utils::{lsp_range_to_std_range, std_range_to_lsp_range},
};
use color_eyre::{eyre::ContextCompat, Result};
use hl7_parser::{parse_message_with_lenient_newlines, Message};
use lsp_textdocument::TextDocuments;
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse, Command,
    Range, Uri,
};
use tracing::instrument;

#[instrument(level = "debug", skip(params, documents))]
pub fn handle_code_actions_request(
    params: CodeActionParams,
    documents: &TextDocuments,
) -> Result<Option<CodeActionResponse>> {
    let uri = params.text_document.uri;
    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;

    let parse_span = tracing::trace_span!("parse message");
    let _parse_span_guard = parse_span.enter();
    let Ok(message) = parse_message_with_lenient_newlines(text) else {
        return Ok(None);
    };
    drop(_parse_span_guard);

    let code_actions = [
        generate_control_id(&params.range, &uri, &message),
        set_time_to_now(&params.range, &uri, &message),
        encode(&params.range, &uri, &message),
        decode(&params.range, &uri, &message),
    ]
    .into_iter()
    .flatten()
    .map(CodeActionOrCommand::CodeAction)
    .collect::<Vec<_>>();

    Ok(Some(code_actions))
}

#[instrument(level = "trace", skip(uri, message))]
fn generate_control_id(range: &Range, uri: &Uri, message: &Message) -> Option<CodeAction> {
    // only available if MSH.10 is present
    message.query("MSH.10").and_then(|existing_control_id| {
        // only if the action range is within the existing control ID
        let action_range = lsp_range_to_std_range(message.raw_value(), *range)?;
        let existing_range = existing_control_id.range();
        if action_range.start < existing_range.start || action_range.end > existing_range.end {
            return None;
        }

        Some(CodeAction {
            title: "Generate new control ID".to_string(),
            kind: Some(CodeActionKind::REFACTOR),
            diagnostics: None,
            edit: None,
            command: Some(Command {
                title: "Generate new control ID".to_string(),
                command: CMD_GENERATE_CONTROL_ID.to_string(),
                arguments: Some(vec![
                    serde_json::to_value(uri.clone()).expect("can serialize uri")
                ]),
            }),
            data: None,
            is_preferred: None,
            disabled: None,
        })
    })
}

#[instrument(level = "trace", skip(uri, message))]
fn set_time_to_now(range: &Range, uri: &Uri, message: &Message) -> Option<CodeAction> {
    let version = message
        .query("MSH.12")
        .map(|msh_12| msh_12.raw_value())
        .unwrap_or("2.7.1");

    tracing::trace!(message_version=?version, "locating cursor");
    let range = lsp_range_to_std_range(message.raw_value(), *range)?;
    let cursor_location = message.locate_cursor(range.start)?;

    let (segment_name, _si, _segment) = cursor_location.segment?;
    let (fi, _field) = cursor_location.field?;
    let (_ri, repeat) = cursor_location.repeat?;

    tracing::trace!(?segment_name, field_index=?fi, "checking if field is a timestamp");
    if spec::is_field_a_timestamp(version, segment_name, fi) {
        tracing::trace!("field is a timestamp, generating code action");
        let range = std_range_to_lsp_range(message.raw_value(), repeat.range.clone());
        Some(CodeAction {
            title: format!("Set {cursor_location} to now"),
            kind: Some(CodeActionKind::REFACTOR),
            diagnostics: None,
            edit: None,
            command: Some(Command {
                title: "Set timestamp to now".to_string(),
                command: CMD_SET_TO_NOW.to_string(),
                arguments: Some(vec![
                    serde_json::to_value(uri.clone()).expect("can serialize uri"),
                    serde_json::to_value(range).expect("can serialize range"),
                ]),
            }),
            data: None,
            is_preferred: None,
            disabled: None,
        })
    } else {
        tracing::trace!("field is not a timestamp");
        None
    }
}

#[instrument(level = "trace", skip(uri, message))]
fn encode(range: &Range, uri: &Uri, message: &Message) -> Option<CodeAction> {
    let selection_range = lsp_range_to_std_range(message.raw_value(), *range)?;
    if selection_range.len() == 0 {
        return None;
    }

    let text = message.raw_value();
    // check if any of the separators are present in the selection, if not, return
    // None
    let separators = message.separators;
    let is_separator = |c: char| {
        separators.field == c
            || separators.component == c
            || separators.subcomponent == c
            || separators.repetition == c
            || separators.escape == c
    };
    let requires_encoding = text[selection_range.clone()].chars().any(is_separator);
    if !requires_encoding {
        return None;
    }

    Some(CodeAction {
        title: "Encode selection".to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: None,
        command: Some(Command {
            title: "Encode selection".to_string(),
            command: CMD_ENCODE_SELECTION.to_string(),
            arguments: Some(vec![
                serde_json::to_value(uri.clone()).expect("can serialize uri"),
                serde_json::to_value(range).expect("can serialize range"),
            ]),
        }),
        is_preferred: None,
        disabled: None,
        data: None,
    })
}

#[instrument(level = "trace", skip(uri, message))]
fn decode(range: &Range, uri: &Uri, message: &Message) -> Option<CodeAction> {
    let selection_range = lsp_range_to_std_range(message.raw_value(), *range)?;
    if selection_range.len() == 0 {
        return None;
    }

    let text = message.raw_value();
    // check if any of the separators are present in the selection, if not, return
    // None
    let escape = message.separators.escape;
    let requires_decoding = text[selection_range.clone()].chars().any(|c| c == escape);
    if !requires_decoding {
        return None;
    }

    Some(CodeAction {
        title: "Decode selection".to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: None,
        command: Some(Command {
            title: "Decode selection".to_string(),
            command: CMD_DECODE_SELECTION.to_string(),
            arguments: Some(vec![
                serde_json::to_value(uri.clone()).expect("can serialize uri"),
                serde_json::to_value(range).expect("can serialize range"),
            ]),
        }),
        is_preferred: None,
        disabled: None,
        data: None,
    })
}
