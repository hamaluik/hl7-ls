use crate::{
    commands::CMD_GENERATE_CONTROL_ID,
    utils::lsp_range_to_std_range,
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

    let code_actions = [generate_control_id(&params.range, &uri, &message)]
        .into_iter()
        .flatten()
        .map(CodeActionOrCommand::CodeAction)
        .collect::<Vec<_>>();

    Ok(Some(code_actions))
}

#[instrument(level = "trace", skip(uri))]
fn generate_control_id(range: &Range, uri: &Uri, message: &Message) -> Option<CodeAction> {
    // only available if MSH.10 is present
    message
        .query("MSH.10")
        .map(|existing_control_id| {
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
        .flatten()
}
