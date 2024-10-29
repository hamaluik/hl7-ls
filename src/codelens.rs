use crate::{commands::CMD_SET_TO_NOW, spec, utils::std_range_to_lsp_range};
use color_eyre::{eyre::ContextCompat, Result};
use hl7_parser::{parse_message_with_lenient_newlines, Message};
use lsp_textdocument::TextDocuments;
use lsp_types::{CodeLens, CodeLensParams, Command, Uri};

pub fn handle_codelens_request(
    params: CodeLensParams,
    documents: &TextDocuments,
) -> Result<Option<Vec<CodeLens>>> {
    let uri = params.text_document.uri;
    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;

    let Ok(_message) = parse_message_with_lenient_newlines(text) else {
        return Ok(None);
    };

    let mut code_lens = vec![];
    code_lens.extend(timestamp_code_lenses(&uri, &_message));

    Ok(Some(code_lens))
}

fn timestamp_code_lenses(uri: &Uri, message: &Message) -> Vec<CodeLens> {
    let message_version = message
        .query("MSH.12")
        .map(|v| v.raw_value())
        .unwrap_or("2.7.1");

    message
        .segments()
        .flat_map(|segment| {
            segment.fields().enumerate().map(|(fi, field)| {
                if spec::is_field_a_timestamp(message_version, segment.name, fi + 1) {
                    let range = std_range_to_lsp_range(message.raw_value(), field.range.clone());
                    let code_lens = CodeLens {
                        range,
                        command: Some(Command {
                            title: format!(
                                "Set {segment}.{field} ({field_value}) to now (UTC)",
                                segment = segment.name,
                                field = fi + 1,
                                field_value = field.raw_value()
                            ),
                            command: CMD_SET_TO_NOW.to_string(),
                            arguments: Some(vec![
                                serde_json::to_value(uri.clone()).expect("can serialize uri"),
                                serde_json::to_value(range).expect("can serialize range"),
                            ]),
                        }),
                        data: None,
                    };
                    Some(code_lens)
                } else {
                    None
                }
            })
        })
        .flatten()
        .collect()
}
