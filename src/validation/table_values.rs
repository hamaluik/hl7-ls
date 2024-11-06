use super::{ValidationCode, ValidationError};
use crate::workspace::specs::WorkspaceSpecs;
use hl7_definitions::table_values;
use hl7_parser::Message;
use lsp_types::{DiagnosticSeverity, Uri};
use tracing::instrument;

#[instrument(level = "debug", skip(message))]
pub fn validate_message(
    uri: &Uri,
    message: &Message,
    version: &str,
    workspace_specs: &Option<&WorkspaceSpecs>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for segment in message.segments() {
        if let Some(segment_definition) = hl7_definitions::get_segment(version, segment.name) {
            for (fi, field) in segment.fields().enumerate() {
                if field.is_empty() {
                    continue;
                }

                let workspace_table_values = workspace_specs
                    .as_ref()
                    .map(|specs| specs.table_values(uri, segment.name, fi + 1))
                    .unwrap_or_default();

                if workspace_table_values.is_empty() {
                    // use the default table values
                    if let Some(field_definition) = segment_definition.fields.get(fi) {
                        if let Some(table) = field_definition.table {
                            if let Some(table_values) = table_values(table as u16) {
                                for repeat in field.repeats() {
                                    if table_values.iter().all(|v| v.0 != repeat.raw_value()) {
                                        errors.push(ValidationError::new(
                                            ValidationCode::InvalidTableValue,
                                            format!(
                                                "Invalid table value, expected one of:\n{table_values}",
                                                table_values = table_values
                                                    .iter()
                                                    .map(|v| format!(
                                                        "  - `{value}` ({description})",
                                                        value = v.0,
                                                        description = v.1
                                                    ))
                                                    .collect::<Vec<String>>()
                                                    .join("\n")
                                            ),
                                            field.range.clone(),
                                            DiagnosticSeverity::INFORMATION,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // use the workspace table values
                    for repeat in field.repeats() {
                        if workspace_table_values
                            .iter()
                            .all(|v| v.0 != repeat.raw_value())
                        {
                            errors.push(ValidationError::new(
                                ValidationCode::InvalidTableValue,
                                format!(
                                    "Invalid table value, expected one of:\n{table_values}",
                                    table_values = workspace_table_values
                                        .iter()
                                        .map(|v| format!(
                                            "  - `{value}` ({description})",
                                            value = v.0,
                                            description = v.1
                                        ))
                                        .collect::<Vec<String>>()
                                        .join("\n")
                                ),
                                field.range.clone(),
                                DiagnosticSeverity::INFORMATION,
                            ));
                        }
                    }
                }
            }
        }
    }

    errors
}
