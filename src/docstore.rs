use lsp_types::{Diagnostic, Uri};
use std::collections::HashMap;

#[derive(Default)]
pub struct DocStore {
    pub docs: HashMap<Uri, String>,
}

impl DocStore {
    /// Update the document store with the given URI and text.
    ///
    /// Returns a list of errors encountered while parsing the document.
    pub fn update(&mut self, uri: Uri, text: String) -> Vec<Diagnostic> {
        let mut result = Vec::default();
        match hl7_parser::parse_message_with_lenient_newlines(text.as_str()) {
            Ok(message) => {
                let errors = crate::validation::validate_message(&message);
                for error in errors {
                    result.push(error.to_diagnostic(text.as_str()));
                }
            }
            Err(e) => {
                result.push(crate::diagnostics::parse_error_to_diagnostic(
                    text.as_str(),
                    e,
                ));
            }
        }

        self.docs.insert(uri.clone(), text);
        result
    }

    pub fn get(&self, uri: &Uri) -> Option<&str> {
        self.docs.get(uri).map(String::as_str)
    }
}

// pub enum DocumentMessage<'m> {
//     Message(Message<'m>),
//     Error(String),
// }
//
// #[derive(Default)]
// pub struct MessageStore<'m> {
//     pub messages: HashMap<Uri, DocumentMessage<'m>>,
// }
//
// impl<'m> MessageStore<'m> {
//     pub fn update(&mut self, uri: Uri, document: &str) {
//         let message = match hl7_parser::parse_message(document) {
//             Ok(message) => DocumentMessage::Message(message),
//             Err(err) => DocumentMessage::Error(err.to_string()),
//         };
//         self.messages.insert(uri, message);
//     }
// }
//
