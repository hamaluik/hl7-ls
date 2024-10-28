use hl7_parser::parser::ParseError;
use lsp_server::{Connection, Message, Notification};
use lsp_types::{
    notification::Notification as _, Diagnostic, DiagnosticSeverity, Position, Range, Uri,
};

use crate::{docstore::DocStore, utils::position_from_offset};

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

pub fn publish_parse_error_diagnostics(
    connection: &Connection,
    doc_store: &DocStore,
    uri: Uri,
    errors: Vec<ParseError>,
    version: i32,
) {
    let text = doc_store.get(&uri).expect("can get text");
    let diagnostics = errors.into_iter().map(|error| {
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
    });
    let publish_diagnostics = lsp_types::PublishDiagnosticsParams {
        uri,
        diagnostics: diagnostics.collect(),
        version: Some(version),
    };
    connection
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::PublishDiagnostics::METHOD.to_string(),
            publish_diagnostics,
        )))
        .expect("can send diagnostics");
}
