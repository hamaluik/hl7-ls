use crate::{spec, utils::std_range_to_lsp_range};
use color_eyre::{
    eyre::{Context, ContextCompat},
    Result,
};
use hl7_parser::{
    message::{Field, Repeat, Segment},
    Message,
};
use lsp_textdocument::TextDocuments;
use lsp_types::{DocumentSymbol, DocumentSymbolParams, SymbolKind};
use tracing::instrument;

#[instrument(level = "debug", skip(params, documents))]
pub fn handle_document_symbols_request(
    params: DocumentSymbolParams,
    documents: &TextDocuments,
) -> Result<Vec<DocumentSymbol>> {
    let uri = params.text_document.uri;
    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {uri:?}"))?;

    let parse_span = tracing::trace_span!("parse message");
    let _parse_span_guard = parse_span.enter();
    let message = hl7_parser::parse_message_with_lenient_newlines(text)
        .wrap_err_with(|| "Failed to parse HL7 message")?;
    drop(_parse_span_guard);

    let mut version = message
        .query("MSH.12")
        .map(|v| v.raw_value())
        .unwrap_or("2.7.1");
    if !spec::is_valid_version(version) {
        version = "2.7.1";
    }

    Ok(segment_symbols(version, &message, text))
}

#[instrument(level = "trace", skip(msg, text))]
fn segment_symbols(version: &str, msg: &Message, text: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    for segment in msg.segments() {
        let name = segment.name.to_string();
        let range = std_range_to_lsp_range(text, segment.range.clone());

        let detail = hl7_definitions::get_segment(version, name.as_str())
            .map(|def| def.description.to_string());

        #[allow(deprecated)]
        let symbol = DocumentSymbol {
            name,
            detail,
            kind: SymbolKind::CLASS,
            tags: None,
            range,
            selection_range: range,
            children: Some(field_symbols(version, segment, text)),
            deprecated: None,
        };
        symbols.push(symbol);
    }

    symbols
}

#[instrument(level = "trace", skip(version, segment, text))]
fn field_symbols(version: &str, segment: &Segment, text: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for (i, field) in segment.fields().enumerate() {
        let name = format!("{segment}.{field}", segment = segment.name, field = i + 1);
        let range = std_range_to_lsp_range(text, field.range.clone());

        let detail = hl7_definitions::get_segment(version, segment.name)
            .and_then(|seg| seg.fields.get(i))
            .map(|f| f.description.to_string());

        #[allow(deprecated)]
        let symbol = DocumentSymbol {
            name,
            detail,
            kind: SymbolKind::FIELD,
            tags: None,
            range,
            selection_range: range,
            children: repeat_symbols(version, segment, (i, field), text),
            deprecated: None,
        };
        symbols.push(symbol);
    }

    symbols
}

#[instrument(level = "trace", skip(version, segment, field, text))]
fn repeat_symbols(
    version: &str,
    segment: &Segment,
    field: (usize, &Field),
    text: &str,
) -> Option<Vec<DocumentSymbol>> {
    match field.1.repeats.len() {
        0 => None,
        1 => {
            let c_symbols =
                component_symbols(version, segment, field, (None, &field.1.repeats[0]), text);
            if c_symbols.is_empty() {
                None
            } else {
                Some(c_symbols)
            }
        }
        _ => Some(
            field
                .1
                .repeats()
                .enumerate()
                .map(|(ri, repeat)| {
                    let name = format!(
                        "{segment}.{field}[{repeat}]",
                        segment = segment.name,
                        field = field.0,
                        repeat = ri + 1
                    );
                    let range = std_range_to_lsp_range(text, repeat.range.clone());

                    let c_symbols =
                        component_symbols(version, segment, field, (Some(ri), repeat), text);

                    #[allow(deprecated)]
                    DocumentSymbol {
                        name,
                        detail: None,
                        kind: SymbolKind::FIELD,
                        tags: None,
                        range,
                        selection_range: range,
                        children: if c_symbols.is_empty() {
                            None
                        } else {
                            Some(c_symbols)
                        },
                        deprecated: None,
                    }
                })
                .collect(),
        ),
    }
}

#[instrument(level = "trace", skip(version, segment, field, repeat, text))]
fn component_symbols(
    version: &str,
    segment: &Segment,
    field: (usize, &Field),
    repeat: (Option<usize>, &Repeat),
    text: &str,
) -> Vec<DocumentSymbol> {
    repeat
        .1
        .components()
        .enumerate()
        .map(|(ci, component)| {
            let repeat_name = repeat
                .0
                .map(|r| format!("[{repeat}]", repeat = r + 1))
                .unwrap_or_default();
            let name = format!(
                "{segment}.{field}{repeat}.{component}",
                segment = segment.name,
                field = field.0,
                repeat = repeat_name,
                component = ci + 1
            );
            let range = std_range_to_lsp_range(text, component.range.clone());

            let detail = hl7_definitions::get_segment(version, segment.name)
                .and_then(|seg| seg.fields.get(field.0))
                .and_then(|f| hl7_definitions::get_field(version, f.datatype))
                .and_then(|f| f.subfields.get(ci))
                .map(|c| c.description.to_string());

            #[allow(deprecated)]
            DocumentSymbol {
                name,
                detail,
                kind: SymbolKind::FIELD,
                tags: None,
                range,
                selection_range: range,
                children: None,
                deprecated: None,
            }
        })
        .collect()
}
