use super::CommandResult;
use color_eyre::{eyre::ContextCompat, Result};
use hl7_parser::parse_message_with_lenient_newlines;
use lsp_textdocument::TextDocuments;
use lsp_types::{ExecuteCommandParams, Uri};
use tracing::instrument;

#[instrument(level = "debug", skip(documents))]
pub fn handle_encode_text_command(
    params: ExecuteCommandParams,
    documents: &TextDocuments,
) -> Result<Option<CommandResult>> {
    if params.arguments.len() < 1 || params.arguments.len() > 2 {
        return Err(color_eyre::eyre::eyre!(
            "Expected 1 or 2 arguments for encode text command"
        ));
    }

    let text = params.arguments[0]
        .as_str()
        .wrap_err("Expected text as first argument")?;

    let uri: Option<Uri> = params
        .arguments
        .get(1)
        .and_then(|v| v.as_str().map(|s| s.parse().ok()).flatten());
    let separators = uri
        .and_then(|uri| documents.get_document_content(&uri, None))
        .and_then(|text| {
            parse_message_with_lenient_newlines(text)
            .ok()
        })
        .map(|message| message.separators.clone())
        .unwrap_or_default();

    let encoded = separators.encode(text).to_string();

    Ok(Some(CommandResult::ValueResponse { value: serde_json::Value::String(encoded) }))
}

#[instrument(level = "debug", skip(documents))]
pub fn handle_decode_text_command(
    params: ExecuteCommandParams,
    documents: &TextDocuments,
) -> Result<Option<CommandResult>> {
    if params.arguments.len() < 1 || params.arguments.len() > 2 {
        return Err(color_eyre::eyre::eyre!(
            "Expected 1 or 2 arguments for decode text command"
        ));
    }

    let text = params.arguments[0]
        .as_str()
        .wrap_err("Expected text as first argument")?;

    let uri: Option<Uri> = params
        .arguments
        .get(1)
        .and_then(|v| v.as_str().map(|s| s.parse().ok()).flatten());
    let separators = uri
        .and_then(|uri| documents.get_document_content(&uri, None))
        .and_then(|text| {
            parse_message_with_lenient_newlines(text)
            .ok()
        })
        .map(|message| message.separators.clone())
        .unwrap_or_default();

    let decoded = separators.decode(text).to_string();
    Ok(Some(CommandResult::ValueResponse { value: serde_json::Value::String(decoded) }))
}
