use crate::spec;
use hl7_parser::Message;
use lsp_types::DiagnosticSeverity;
use tracing::instrument;

use super::{ValidationCode, ValidationError};

#[instrument(level = "debug", skip(message))]
pub fn validate_message<'m>(message: &'m Message) -> (Option<&'m str>, Vec<ValidationError>) {
    let version_range = message.query("MSH.12").map(|v| (v.raw_value(), v.range()));

    let mut errors = Vec::new();
    if let Some((version, range)) = version_range.as_ref() {
        if !spec::is_valid_version(version) {
            errors.push(ValidationError::new(
                ValidationCode::InvalidTableValue,
                format!("Unknown HL7 version `{}`", version),
                range.clone(),
                DiagnosticSeverity::WARNING,
            ));
        }
    }

    // TODO: more MSH errors

    (version_range.map(|v| v.0), errors)
}
