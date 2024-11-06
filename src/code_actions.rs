use std::collections::HashMap;

use color_eyre::{eyre::ContextCompat, Result};
use hl7_parser::{parse_message_with_lenient_newlines, Message};
use lsp_textdocument::TextDocuments;
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    TextEdit, Uri, WorkspaceEdit,
};
use tracing::instrument;

use crate::utils::std_range_to_lsp_range;

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

    let code_actions = [generate_control_id(&uri, &message)]
        .into_iter()
        .flatten()
        .map(CodeActionOrCommand::CodeAction)
        .collect::<Vec<_>>();

    Ok(Some(code_actions))
}

#[instrument(level = "trace", skip(uri))]
fn generate_control_id(uri: &Uri, message: &Message) -> Option<CodeAction> {
    message.query("MSH.10").map(|control_id| {
        use rand::distributions::{Alphanumeric, DistString};
        let new_control_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 20);

        let range = control_id.range();
        #[allow(clippy::mutable_key_type)]
        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
        changes.insert(
            uri.clone(),
            vec![TextEdit {
                range: std_range_to_lsp_range(message.raw_value(), range),
                new_text: new_control_id,
            }],
        );

        CodeAction {
            title: "Generate new control ID".to_string(),
            kind: Some(CodeActionKind::REFACTOR),
            diagnostics: None,
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                ..Default::default()
            }),
            command: None,
            data: None,
            is_preferred: None,
            disabled: None,
        }
    })
}
