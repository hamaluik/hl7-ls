use hl7_parser::parser::ParseError;
use lsp_server::{Connection, Message, Notification};
use lsp_types::{
    notification::Notification as _, Diagnostic, DiagnosticSeverity, Position, Range, Uri,
};

use crate::utils::position_from_offset;

pub fn clear_diagnostics(connection: &Connection, uri: Uri) {
    let publish_diagnostics = lsp_types::PublishDiagnosticsParams {
        uri,
        diagnostics: Vec::new(),
        version: None,
    };
    connection
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::PublishDiagnostics::METHOD.to_string(),
            publish_diagnostics,
        )))
        .expect("can send diagnostics");
}

pub fn parse_error_to_diagnostic(text: &str, error: ParseError) -> Diagnostic {
    let message = error.to_string();
    let pos = match error {
        ParseError::FailedToParse {
            position: offset, ..
        } => position_from_offset(text, offset),
        ParseError::IncompleteInput(_) => position_from_offset(text, text.len()),
    };

    Diagnostic {
        range: Range {
            start: pos,
            end: Position {
                line: pos.line,
                character: pos.character + 1,
            },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        message,
        ..Default::default()
    }
}

pub fn publish_parse_error_diagnostics(
    connection: &Connection,
    uri: Uri,
    diagnostics: Vec<Diagnostic>,
    version: Option<i32>,
) {
    let publish_diagnostics = lsp_types::PublishDiagnosticsParams {
        uri,
        diagnostics,
        version,
    };
    connection
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::PublishDiagnostics::METHOD.to_string(),
            publish_diagnostics,
        )))
        .expect("can send diagnostics");
}
