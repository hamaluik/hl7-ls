use super::{ValidationCode, ValidationError};
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
                    if field_definition.datatype == "TS" {
                        if let Err(e) = hl7_parser::timestamps::parse_timestamp(field.raw_value()) {
                            errors.push(ValidationError::new(
                                ValidationCode::InvalidTimestamp,
                                format!("Invalid timestamp: {e:#}"),
                                field.range.clone(),
                                DiagnosticSeverity::WARNING,
                            ));
                        }
                    }
                }
            }
        }
    }

    errors
}
