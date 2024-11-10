use crate::utils::{position_to_offset, std_range_to_lsp_range};
use color_eyre::{eyre::ContextCompat, Result};
use hl7_parser::{locate::LocatedCursor, parse_message_with_lenient_newlines};
use lsp_textdocument::TextDocuments;
use lsp_types::{SelectionRange, SelectionRangeParams};
use tracing::instrument;

#[instrument(level = "debug", skip(params, documents))]
pub fn handle_selection_range_request(
    params: SelectionRangeParams,
    documents: &TextDocuments,
) -> Result<Vec<SelectionRange>> {
    let uri = params.text_document.uri;
    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;

    let parse_span = tracing::trace_span!("parse message");
    let _parse_span_guard = parse_span.enter();
    let Ok(message) = parse_message_with_lenient_newlines(text) else {
        let mut ranges = Vec::with_capacity(params.positions.len());
        for _ in 0..params.positions.len() {
            ranges.push(SelectionRange {
                range: lsp_types::Range::default(),
                parent: None,
            });
        }
        return Ok(ranges);
    };
    drop(_parse_span_guard);

    Ok(params
        .positions
        .into_iter()
        .map(|position| {
            let location =
                position_to_offset(message.raw_value(), position.line, position.character)
                    .and_then(|offset| message.locate_cursor(offset))?;

            let LocatedCursor {
                segment,
                field,
                repeat,
                component,
                sub_component,
                ..
            } = location;
            let segment = segment?.2;

            let range = SelectionRange {
                range: std_range_to_lsp_range(message.raw_value(), segment.range.clone()),
                parent: None,
            };

            let range = match field.map(|f| f.1) {
                Some(field) => SelectionRange {
                    range: std_range_to_lsp_range(message.raw_value(), field.range.clone()),
                    parent: Some(Box::new(range)),
                },
                None => range,
            };

            let range = match repeat.map(|r| r.1) {
                Some(repeat) => SelectionRange {
                    range: std_range_to_lsp_range(message.raw_value(), repeat.range.clone()),
                    parent: Some(Box::new(range)),
                },
                None => range,
            };

            let range = match component.map(|c| c.1) {
                Some(component) => SelectionRange {
                    range: std_range_to_lsp_range(message.raw_value(), component.range.clone()),
                    parent: Some(Box::new(range)),
                },
                None => range,
            };

            let range = match sub_component.map(|s| s.1) {
                Some(sub_component) => SelectionRange {
                    range: std_range_to_lsp_range(message.raw_value(), sub_component.range.clone()),
                    parent: Some(Box::new(range)),
                },
                None => range,
            };

            Some(range)
        })
        .map(|range| {
            range.unwrap_or_else(|| SelectionRange {
                range: lsp_types::Range::default(),
                parent: None,
            })
        })
        .collect())
}
