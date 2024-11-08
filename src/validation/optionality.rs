use crate::workspace::specs::WorkspaceSpecs;

use super::ValidationError;
use hl7_definitions::FieldOptionality;
use hl7_parser::Message;
use lsp_types::DiagnosticSeverity;
use tracing::instrument;

#[instrument(level = "debug", skip(message))]
pub fn validate_message(
    message: &Message,
    version: &str,
    workspace_specs: &Option<&WorkspaceSpecs>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for segment in message.segments() {
        if let Some(segment_definition) = hl7_definitions::get_segment(version, segment.name) {
            for (fi, field) in segment.fields().enumerate() {
                for repeat in field.repeats() {
                    // workspace fields
                    if let Some(workspace_specs) = *workspace_specs {
                        if repeat.is_empty()
                            && workspace_specs.is_field_required(segment.name, fi + 1)
                        {
                            errors.push(ValidationError::new(
                                super::ValidationCode::InvalidOptionality,
                                "Field is required".to_string(),
                                field.range.clone(),
                                DiagnosticSeverity::WARNING,
                            ));
                        }
                    }

                    // standard fields
                    if let Some(field_definition) = segment_definition.fields.get(fi) {
                        if field_definition.optionality == FieldOptionality::Required
                            && repeat.is_empty()
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
    }

    errors
}
