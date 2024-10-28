use super::{ValidationCode, ValidationError};
use hl7_parser::Message;
use lsp_types::DiagnosticSeverity;

pub fn validate_message(message: &Message, version: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for segment in message.segments() {
        if let Some(segment_definition) = hl7_definitions::get_segment(version, segment.name) {
            for (fi, field) in segment.fields().enumerate() {
                if field.repeats().next().map(|r| r.components().count() > 1) == Some(true) {
                    continue;
                }
                if let Some(field_definition) = segment_definition.fields.get(fi) {
                    if let Some(max_length) = field_definition.max_length {
                        if field.raw_value().len() > max_length {
                            errors.push(ValidationError::new(
                                ValidationCode::InvalidLength,
                                format!("Field is too long (max: {})", max_length),
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
