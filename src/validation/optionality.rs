use super::ValidationError;
use hl7_definitions::FieldOptionality;
use hl7_parser::Message;
use lsp_types::DiagnosticSeverity;
use tracing::instrument;

#[instrument(level = "debug", skip(message))]
pub fn validate_message(message: &Message, version: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for segment in message.segments() {
        if let Some(segment_definition) = hl7_definitions::get_segment(version, segment.name) {
            for (fi, field) in segment.fields().enumerate() {
                if let Some(field_definition) = segment_definition.fields.get(fi) {
                    if field_definition.optionality == FieldOptionality::Required
                        && field.is_empty()
                    {
                        errors.push(ValidationError::new(
                            super::ValidationCode::InvalidOptionality,
                            "Field is required".to_string(),
                            field.range.clone(),
                            DiagnosticSeverity::WARNING,
                        ));
                    }
                }
            }
        }
    }

    errors
}
