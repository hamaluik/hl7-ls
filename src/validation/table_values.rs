use super::{ValidationCode, ValidationError};
use hl7_definitions::table_values;
use hl7_parser::Message;
use lsp_types::DiagnosticSeverity;

pub fn validate_message(message: &Message, version: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for segment in message.segments() {
        if let Some(segment_definition) = hl7_definitions::get_segment(version, segment.name) {
            for (fi, field) in segment.fields().enumerate() {
                if field.is_empty() {
                    continue;
                }
                if let Some(field_definition) = segment_definition.fields.get(fi) {
                    if let Some(table) = field_definition.table {
                        if let Some(table_values) = table_values(table as u16) {
                            if table_values.iter().all(|v| v.0 != field.raw_value()) {
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
        }
    }

    errors
}
