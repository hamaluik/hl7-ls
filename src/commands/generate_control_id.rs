use super::CommandResult;
use crate::utils::std_range_to_lsp_range;
use color_eyre::{
    eyre::{Context, ContextCompat},
    Result,
};
use hl7_parser::parse_message_with_lenient_newlines;
use lsp_textdocument::TextDocuments;
use lsp_types::{ExecuteCommandParams, TextEdit, Uri, WorkspaceEdit};
use std::collections::HashMap;
use tracing::instrument;

#[instrument(level = "debug", skip(documents))]
pub fn handle_generate_control_id_command(
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
