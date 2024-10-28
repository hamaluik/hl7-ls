use crate::utils::position_from_offset;
use hl7_parser::Message;
use lsp_types::{Diagnostic, DiagnosticSeverity};
use std::{fmt, ops::Range};

mod length;
mod msh;
mod optionality;
mod table_values;
mod timestamps;

#[derive(Debug, Copy, Clone)]
pub enum ValidationCode {
    MessageStructure,
    InvalidTableValue,
    InvalidTimestamp,
    InvalidLength,
    InvalidOptionality,
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

    pub fn to_diagnostic(self, text: &str) -> Diagnostic {
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

pub fn validate_message(message: &Message) -> Vec<ValidationError> {
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
    errors.extend(timestamps::validate_message(message, version));
    errors.extend(length::validate_message(message, version));
    errors.extend(table_values::validate_message(message, version));

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
        }
    }
}
