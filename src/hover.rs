use crate::{
    spec,
    utils::{position_to_offset, range_from_offsets},
    workspace::specs::WorkspaceSpecs,
};
use chrono::{DateTime, Local, Utc};
use color_eyre::{
    eyre::{Context, ContextCompat},
    Result,
};
use hl7_parser::parse_message_with_lenient_newlines;
use lsp_textdocument::TextDocuments;
use lsp_types::{Hover, HoverContents, HoverParams, MarkedString};
use tracing::instrument;

#[instrument(level = "debug", skip(params, documents, workspace_specs))]
pub fn handle_hover_request(
    params: HoverParams,
    documents: &TextDocuments,
    workspace_specs: Option<&WorkspaceSpecs>,
) -> Result<Hover> {
    let uri = params.text_document_position_params.text_document.uri;
    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;
    let position = params.text_document_position_params.position;
    let offset = position_to_offset(text, position.line, position.character)
        .wrap_err_with(|| "Failed to convert position to offset")?;

    let parse_span = tracing::trace_span!("parse message");
    let _parse_span_guard = parse_span.enter();
    let message = parse_message_with_lenient_newlines(text)
        .wrap_err_with(|| "Failed to parse HL7 message")?;
    drop(_parse_span_guard);

    let locate_span = tracing::trace_span!("locate cursor");
    let _locate_span_guard = locate_span.enter();
    let location = message
        .locate_cursor(offset)
        .wrap_err_with(|| "Failed to locate cursor in HL7 message")?;
    drop(_locate_span_guard);

    // format the hover text
    let format_span = tracing::trace_span!("format hover text");
    let _format_span_guard = format_span.enter();
    let mut hover_text = format!("`{location}`");
    let mut url = None;
    let mut timestamp = None;
    let mut has_workspace_description = false;
    if let Some(seg) = location.segment {
        let message_version = message
            .query("MSH.12")
            .map(|v| v.raw_value())
            .unwrap_or("2.7.1");
        if !spec::is_valid_version(message_version) {
            hover_text.push_str(format!("\n\nUnknown HL7 version `{}`", message_version).as_str());
        }

        let description = spec::segment_description(message_version, seg.0);
        hover_text.push_str(format!(":\n  {segment}: {description}", segment = seg.0).as_str());

        if let Some(field) = location.field {
            let field_description = spec::describe_field(message_version, seg.0, field.0);

            let has_repeats = field.1.has_repeats();
            let repeat = if has_repeats {
                let repeat = location.repeat.map(|r| r.0).unwrap_or(0);
                format!("[{repeat}]")
            } else {
                "".to_string()
            };

            let has_components = location
                .repeat
                .map(|r| r.1.has_components())
                .unwrap_or(false);

            if let Some(workspace_specs) = workspace_specs {
                let workspace_description = workspace_specs.describe_field(&uri, seg.0, field.0);
                if !workspace_description.is_empty() {
                    hover_text.push_str(
                        format!(
                            "\n  **{segment}.{field}{repeat}**: {workspace_description}†",
                            segment = seg.0,
                            field = field.0,
                            repeat = repeat,
                            workspace_description = workspace_description,
                        )
                        .as_str(),
                    );
                    has_workspace_description = true;
                }
            }

            hover_text.push_str(
                format!(
                    "\n  {segment}.{field}{repeat}: {field_description}",
                    segment = seg.0,
                    field = field.0,
                )
                .as_str(),
            );

            if let (true, Some(component)) = (has_components, location.component) {
                let component_description =
                    spec::describe_component(message_version, seg.0, field.0, component.0);
                hover_text.push_str(
                    format!(
                        "\n  {segment}.{field}.{component}: {component_description}",
                        segment = seg.0,
                        field = field.0,
                        component = component.0,
                    )
                    .as_str(),
                );

                url = Some(format!(
                        "https://hl7-definition.caristix.com/v2/HL7v{message_version}/Fields/{segment}.{field}.{component}",
                        segment = seg.0,
                        field = field.0,
                        component = component.0
                    ));

                if spec::is_component_a_timestamp(message_version, seg.0, field.0, component.0) {
                    timestamp = Some(
                        match hl7_parser::timestamps::parse_timestamp(component.1.raw_value()) {
                            Ok(ts) => {
                                let ts_utc = ts
                                    .try_into()
                                    .map(|ts: DateTime<Utc>| ts.to_rfc2822())
                                    .unwrap_or_else(|e| {
                                        format!("Failed to parse timestamp as UTC: {e:#}")
                                    });
                                let ts_local = ts
                                    .try_into()
                                    .map(|ts: DateTime<Local>| ts.to_rfc2822())
                                    .unwrap_or_else(|e| {
                                        format!("Failed to parse timestamp as local: {e:#}")
                                    });
                                format!("  UTC: `{ts_utc}`\n  Local: `{ts_local}`")
                            }
                            Err(e) => format!("Invalid timestamp: {e:#}"),
                        },
                    );
                }
            } else {
                url = Some(format!(
                        "https://hl7-definition.caristix.com/v2/HL7v{message_version}/Fields/{segment}.{field}",
                        segment = seg.0,
                        field = field.0
                    ));

                if spec::is_field_a_timestamp(message_version, seg.0, field.0) {
                    timestamp = Some(
                        match hl7_parser::timestamps::parse_timestamp(field.1.raw_value()) {
                            Ok(ts) => {
                                let ts_utc = ts
                                    .try_into()
                                    .map(|ts: DateTime<Utc>| ts.to_rfc2822())
                                    .unwrap_or_else(|e| {
                                        format!("Failed to parse timestamp as UTC: {e:#}")
                                    });
                                let ts_local = ts
                                    .try_into()
                                    .map(|ts: DateTime<Local>| ts.to_rfc2822())
                                    .unwrap_or_else(|e| {
                                        format!("Failed to parse timestamp as local: {e:#}")
                                    });
                                format!("  UTC: `{ts_utc}`\n  Local: `{ts_local}`")
                            }
                            Err(e) => format!("Invalid timestamp: {e:#}"),
                        },
                    );
                }
            }
        } else {
            url = Some(format!(
                "https://hl7-definition.caristix.com/v2/HL7v{message_version}/Segments/{segment}",
                segment = seg.0
            ));
        }
    }

    if url.is_some() || timestamp.is_some() || has_workspace_description {
        hover_text.push_str("\n\n---");
    }

    if let Some(timestamp) = timestamp {
        hover_text.push_str(format!("\n\n**Timestamp**:\n{timestamp}").as_str());
    }
    if let Some(url) = url {
        hover_text.push_str(format!("\n\n**More info**: [{url}]({url})").as_str());
    }
    if has_workspace_description {
        hover_text.push_str("\n\n†: Workspace description");
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

    drop(_format_span_guard);
    tracing::trace!(hover_text = %hover_text, range = ?range, "generated hover text");

    let hover = Hover {
        contents: HoverContents::Scalar(MarkedString::from_markdown(hover_text)),
        range,
    };

    Ok(hover)
}
