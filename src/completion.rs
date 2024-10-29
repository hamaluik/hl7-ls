use color_eyre::{eyre::ContextCompat, Result};
use hl7_parser::parse_message_with_lenient_newlines;
use lsp_textdocument::TextDocuments;
use lsp_types::{CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse};

use crate::{spec, utils::position_to_offset};

pub fn handle_completion_request(
    params: CompletionParams,
    documents: &TextDocuments,
) -> Result<CompletionResponse> {
    let uri = params.text_document_position.text_document.uri;
    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;
    let position = params.text_document_position.position;
    let offset = position_to_offset(text, position.line, position.character)
        .wrap_err_with(|| "Failed to convert position to offset")?;

    let mut completions = vec![];

    if let Ok(message) = parse_message_with_lenient_newlines(text) {
        let version = message
            .query("MSH.12")
            .map(|v| v.raw_value())
            .unwrap_or("2.7.1");

        if let Some(location) = message.locate_cursor(offset) {
            if let Some((segment_name, _si, _segment)) = location.segment {
                if let Some((fi, _field)) = location.field {
                    let has_components = location
                        .repeat
                        .map(|r| r.1.has_components())
                        .unwrap_or(false);
                    if has_components {
                        if let Some(table_values) = spec::component_table_values(
                            version,
                            segment_name,
                            fi - 1,
                            location.component.unwrap().0 - 1,
                        ) {
                            tracing::trace!(?table_values, "found component table values");
                            completions.extend(table_values.into_iter().map(|v| {
                                let (label, detail) = v;
                                lsp_types::CompletionItem {
                                    label,
                                    label_details: Some(lsp_types::CompletionItemLabelDetails {
                                        detail,
                                        description: None,
                                    }),
                                    kind: Some(CompletionItemKind::VALUE),
                                    ..Default::default()
                                }
                            }));
                        } else {
                            tracing::trace!("no component table values found");
                        }
                    } else if let Some(table_values) =
                        spec::field_table_values(version, segment_name, fi)
                    {
                        tracing::trace!(?table_values, "found field table values");
                        completions.extend(table_values.into_iter().map(|v| {
                            let (label, detail) = v;

                            lsp_types::CompletionItem {
                                label,
                                label_details: Some(lsp_types::CompletionItemLabelDetails {
                                    detail,
                                    description: None,
                                }),
                                kind: Some(CompletionItemKind::VALUE),
                                ..Default::default()
                            }
                        }));
                    } else {
                        tracing::trace!("no field table values found");
                    }
                }
            }
        }
    }

    if completions.is_empty() && position.character < 3 {
        completions.extend(segment_completions("2.7.1"));
    }

    Ok(CompletionResponse::Array(completions))
}

fn segment_completions(version: &str) -> Vec<CompletionItem> {
    hl7_definitions::get_definition(version)
        .map(|def| {
            def.segments
                .keys()
                .map(|s| CompletionItem {
                    label: s.to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    ..Default::default()
                })
                .collect()
        })
        .unwrap_or_default()
}
