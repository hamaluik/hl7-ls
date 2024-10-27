use lsp_types::Uri;
use std::collections::HashMap;

#[derive(Default)]
pub struct DocStore {
    pub docs: HashMap<Uri, String>,
}

impl DocStore {
    /// Update the document store with the given URI and text.
    ///
    /// Returns a list of errors encountered while parsing the document.
    pub fn update(&mut self, uri: Uri, text: String) -> Vec<String> {
        let mut result = Vec::default();
        if let Err(e) = hl7_parser::parse_message_with_lenient_newlines(text.as_str()) {
            result.push(e.to_string());
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
