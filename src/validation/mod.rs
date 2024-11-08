use crate::{utils::position_from_offset, workspace::specs::WorkspaceSpecs, Opts};
use hl7_parser::Message;
use lsp_types::{Diagnostic, DiagnosticSeverity, Uri};
use std::{fmt, ops::Range};
use tracing::instrument;

mod datatypes;
mod length;
mod msh;
mod optionality;
mod table_values;

#[derive(Debug, Copy, Clone)]
pub enum ValidationCode {
    MessageStructure,
    InvalidTableValue,
    InvalidTimestamp,
    InvalidLength,
    InvalidOptionality,
    InvalidDataType(&'static str),
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub code: ValidationCode,
    pub message: String,
    pub range: Range<usize>,
    pub severity: DiagnosticSeverity,
}

impl ValidationError {
    pub fn new(
        code: ValidationCode,
        message: String,
        range: Range<usize>,
        severity: DiagnosticSeverity,
    ) -> Self {
        ValidationError {
            code,
            message,
            range,
            severity,
        }
    }

    pub fn into_diagnostic(self, text: &str) -> Diagnostic {
        Diagnostic {
            range: lsp_types::Range {
                start: position_from_offset(text, self.range.start),
                end: position_from_offset(text, self.range.end),
            },
            severity: Some(self.severity),
            message: self.message,
            code: Some(lsp_types::NumberOrString::String(self.code.to_string())),
            ..Default::default()
        }
    }
}

#[instrument(level = "debug", skip(message, workspace_specs, opts))]
pub fn validate_message(
    uri: &Uri,
    message: &Message,
    workspace_specs: &Option<&WorkspaceSpecs>,
    opts: &Opts,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    if message.segments().count() < 2 {
        errors.push(ValidationError::new(
            ValidationCode::MessageStructure,
            "Message must have at least 2 segments".to_string(),
            0..0,
            DiagnosticSeverity::WARNING,
        ));
    }

    let (version, msh_errors) = msh::validate_message(message);
    let version = version.unwrap_or("2.7.1");
    errors.extend(msh_errors);

    // TODO: these all iterate over the message multiple times; maybe it would
    // be more performant to iterate once and check each rule at the same time?
    errors.extend(optionality::validate_message(message, version));
    errors.extend(length::validate_message(message, version));
    errors.extend(table_values::validate_message(
        uri,
        message,
        version,
        workspace_specs,
        opts,
    ));
    errors.extend(datatypes::validate_message(message, version));
    // TODO: message schema validation

    errors
}

impl fmt::Display for ValidationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationCode::MessageStructure => write!(f, "message structure"),
            ValidationCode::InvalidTableValue => write!(f, "table value"),
            ValidationCode::InvalidTimestamp => write!(f, "timestamp"),
            ValidationCode::InvalidLength => write!(f, "length"),
            ValidationCode::InvalidOptionality => write!(f, "optionality"),
            ValidationCode::InvalidDataType(description) => write!(f, "data type ({description})"),
        }
    }
}
