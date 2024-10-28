use crate::{
    docstore::DocStore,
    utils::{position_to_offset, range_from_offsets},
    spec,
};
use color_eyre::{
    eyre::{Context, ContextCompat},
    Result,
};
use hl7_parser::parse_message_with_lenient_newlines;
use lsp_types::{Hover, HoverContents, HoverParams, MarkedString};

pub fn handle_hover_request(params: HoverParams, doc_store: &DocStore) -> Result<Hover> {
    let uri = params.text_document_position_params.text_document.uri;
    let text = doc_store
        .get(&uri)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;
    let position = params.text_document_position_params.position;
    let offset = position_to_offset(text, position.line, position.character)
        .wrap_err_with(|| "Failed to convert position to offset")?;

    let message = parse_message_with_lenient_newlines(text)
        .wrap_err_with(|| "Failed to parse HL7 message")?;
    let location = message
        .locate_cursor(offset)
        .wrap_err_with(|| "Failed to locate cursor in HL7 message")?;

    // format the hover text
    let mut hover_text = format!("`{location}`");
    if let Some(seg) = location.segment {
        let description = spec::HL7_SEGMENT_DESCRIPTION
            .get(seg.0).copied()
            .unwrap_or("No description found");
        hover_text.push_str(format!(":\n  {segment}: {description}", segment = seg.0).as_str());

        if let Some(field) = location.field {
            let field_description = spec::HL7_FIELD_DESCRIPTION
                .get(seg.0)
                .and_then(|m| m.get(&(field.0 as u32))).copied()
                .unwrap_or("No description found");
            hover_text.push_str(
                format!(
                    "\n  {segment}.{field}: {field_description}",
                    segment = seg.0,
                    field = field.0,
                )
                .as_str(),
            );
        }
    }

    // figure out the most relevant hover range
    let range = if let Some(sub_component) = location.sub_component {
        let start = sub_component.1.range.start;
        let end = sub_component.1.range.end;
        Some(range_from_offsets(text, start, end))
    } else if let Some(component) = location.component {
        let start = component.1.range.start;
        let end = component.1.range.end;
        Some(range_from_offsets(text, start, end))
    } else if let Some(repeat) = location.repeat {
        let start = repeat.1.range.start;
        let end = repeat.1.range.end;
        Some(range_from_offsets(text, start, end))
    } else if let Some(field) = location.field {
        let start = field.1.range.start;
        let end = field.1.range.end;
        Some(range_from_offsets(text, start, end))
    } else if let Some(segment) = location.segment {
        let start = segment.2.range.start;
        let end = segment.2.range.end;
        Some(range_from_offsets(text, start, end))
    } else {
        None
    };

    let hover = Hover {
        contents: HoverContents::Scalar(MarkedString::from_markdown(hover_text)),
        range,
    };

    Ok(hover)
}
