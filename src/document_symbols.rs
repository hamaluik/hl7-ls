use crate::{docstore::DocStore, utils::std_range_to_lsp_range};
use color_eyre::{
    eyre::{Context, ContextCompat},
    Result,
};
use hl7_parser::{message::Segment, Message};
use lsp_types::{DocumentSymbol, DocumentSymbolParams, SymbolKind};

pub fn handle_document_symbols_request(
    params: DocumentSymbolParams,
    doc_store: &DocStore,
) -> Result<Vec<DocumentSymbol>> {
    let uri = params.text_document.uri;
    let text = doc_store
        .get(&uri)
        .wrap_err_with(|| format!("no document found for uri: {uri:?}"))?;

    let message = hl7_parser::parse_message_with_lenient_newlines(text)
        .wrap_err_with(|| "Failed to parse HL7 message")?;

    Ok(segment_symbols(&message, text))
}

fn segment_symbols(msg: &Message, text: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    for segment in msg.segments() {
        let name = segment.name.to_string();
        let range = std_range_to_lsp_range(text, segment.range.clone());

        #[allow(deprecated)]
        let symbol = DocumentSymbol {
            name,
            detail: None, // TODO: description from spec
            kind: SymbolKind::CLASS,
            tags: None,
            range,
            selection_range: range,
            children: Some(field_symbols(segment, text)),
            deprecated: None,
        };
        symbols.push(symbol);
    }

    symbols
}

fn field_symbols(segment: &Segment, text: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for (i, field) in segment.fields().enumerate() {
        let name = format!("{segment}.{field}", segment = segment.name, field = i + 1);
        let range = std_range_to_lsp_range(text, field.range.clone());

        #[allow(deprecated)]
        let symbol = DocumentSymbol {
            name,
            detail: None,
            kind: SymbolKind::FIELD,
            tags: None,
            range,
            selection_range: range,
            children: None, // TODO
            deprecated: None,
        };
        symbols.push(symbol);
    }

    symbols
}
