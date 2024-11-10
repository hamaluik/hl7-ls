use crate::utils::position_to_offset;
use color_eyre::{eyre::ContextCompat, Result};
use hl7_parser::{locate::LocatedCursor, message::Segment, parse_message_with_lenient_newlines};
use lsp_textdocument::TextDocuments;
use lsp_types::{
    ParameterInformation, ParameterLabel, SignatureHelp, SignatureHelpParams, SignatureInformation,
};
use tracing::instrument;

#[instrument(level = "debug", skip(params, documents))]
pub fn handle_signature_help_request(
    params: SignatureHelpParams,
    documents: &TextDocuments,
) -> Result<Option<SignatureHelp>> {
    let uri = params.text_document_position_params.text_document.uri;
    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;

    let parse_span = tracing::trace_span!("parse message");
    let _parse_span_guard = parse_span.enter();
    let Ok(message) = parse_message_with_lenient_newlines(text) else {
        return Ok(None);
    };
    drop(_parse_span_guard);

    let position = params.text_document_position_params.position;
    let offset = position_to_offset(text, position.line, position.character)
        .wrap_err_with(|| "Failed to convert position to offset")?;
    let Some(location) = message.locate_cursor(offset) else {
        return Ok(None);
    };

    let version = message
        .query("MSH.12")
        .map(|v| v.raw_value())
        .unwrap_or("2.7.1");

    let LocatedCursor {
        segment,
        field,
        repeat,
        component,
        ..
    } = location;

    if segment.is_none() || field.is_none() {
        return Ok(None);
    }
    let segment = segment.unwrap().2;
    let field = field.unwrap();

    let Some(segment_signature) =
        build_segment_signature(version, message.separators.field, segment, field.0)
    else {
        return Ok(None);
    };
    let mut signatures = vec![segment_signature];

    let mut active_signature = 0;
    if let Some((ci, _component)) = component {
        if let Some(field_signature) =
            build_field_signature(version, message.separators.component, segment, field.0, ci)
        {
            signatures.push(field_signature);
            if let Some((_ri, repeat)) = repeat {
                if repeat.has_components() {
                    active_signature = 1;
                }
            }
        }
    }

    Ok(Some(SignatureHelp {
        signatures,
        active_signature: Some(active_signature),
        active_parameter: None,
    }))
}

fn build_segment_signature(
    version: &str,
    field_separator: char,
    segment: &Segment,
    current_field: usize,
) -> Option<SignatureInformation> {
    let mut signature_label = format!(
        "{segment_name}{field_separator}",
        segment_name = segment.name
    );
    let field_list = crate::spec::segment_parameters(version, segment.name)?;
    let mut field_parameters: Vec<[u32; 2]> = vec![];
    let mut parameter_start = signature_label.len();
    for parameter in field_list.into_iter() {
        let parameter_end = parameter_start + parameter.len();
        field_parameters.push([parameter_start as u32, parameter_end as u32]);
        signature_label.push_str(&parameter);
        signature_label.push(field_separator);
        parameter_start = parameter_end + 1;
    }

    Some(SignatureInformation {
        label: signature_label,
        documentation: None,
        parameters: Some(
            field_parameters
                .into_iter()
                .map(|parameter_range| ParameterInformation {
                    label: ParameterLabel::LabelOffsets(parameter_range),
                    documentation: None,
                })
                .collect(),
        ),
        active_parameter: Some(current_field as u32 - 1),
    })
}

fn build_field_signature(
    version: &str,
    component_separator: char,
    segment: &Segment,
    field: usize,
    current_component: usize,
) -> Option<SignatureInformation> {
    let mut signature_label = format!(
        "{segment_name}.{field}|",
        segment_name = segment.name,
        field = field
    );
    let component_list = crate::spec::field_parameters(version, segment.name, field)?;
    let mut component_parameters: Vec<[u32; 2]> = vec![];
    let mut parameter_start = signature_label.len();
    for parameter in component_list.into_iter() {
        let parameter_end = parameter_start + parameter.len();
        component_parameters.push([parameter_start as u32, parameter_end as u32]);
        signature_label.push_str(&parameter);
        signature_label.push(component_separator);
        parameter_start = parameter_end + 1;
    }

    Some(SignatureInformation {
        label: signature_label,
        documentation: None,
        parameters: Some(
            component_parameters
                .into_iter()
                .map(|parameter_range| ParameterInformation {
                    label: ParameterLabel::LabelOffsets(parameter_range),
                    documentation: None,
                })
                .collect(),
        ),
        active_parameter: Some(current_component as u32 - 1),
    })
}
