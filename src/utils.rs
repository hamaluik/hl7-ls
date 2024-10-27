use color_eyre::Result;
use lsp_server::{RequestId, Response, ResponseError};
use lsp_types::{Position, Range};
use serde::Serialize;

pub fn position_to_offset(text: &str, line: u32, column: u32) -> Option<usize> {
    let mut offset = 0;
    let mut current_line = 0;
    let mut current_column = 0;
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if current_line == line {
            if current_column == column {
                return Some(offset);
            }
            current_column += 1;
        }

        if c == '\n' {
            current_line += 1;
            current_column = 0;
            offset += 1;
        } else if c == '\r' {
            current_line += 1;
            current_column = 0;
            offset += 1;
            if let Some(&'\n') = chars.peek() {
                chars.next();
                offset += 1;
            }
        } else {
            offset += 1;
        }
    }

    None
}

pub fn position_from_offset(text: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut character = 0;
    let mut chars = text.chars().peekable();

    let mut i = 0;
    while let Some(c) = chars.next() {
        if i == offset {
            break;
        }

        if c == '\n' {
            line += 1;
            character = 0;
        } else if c == '\r' {
            line += 1;
            character = 0;
            if let Some('\n') = chars.peek() {
                chars.next();
                i += 1;
            }
        } else {
            character += 1;
        }

        i += 1;
    }

    Position { line, character }
}

pub fn range_from_offsets(text: &str, start: usize, end: usize) -> Range {
    Range {
        start: position_from_offset(text, start),
        end: position_from_offset(text, end),
    }
}

pub fn build_response<R: Serialize>(id: RequestId, result: Result<R>) -> Response {
    let (result, error) = match result {
        Ok(result) => (
            Some(serde_json::to_value(result).expect("can serialize response")),
            None,
        ),
        Err(error) => (
            None,
            Some(ResponseError {
                code: lsp_server::ErrorCode::InternalError as i32,
                message: error.to_string(),
                data: None,
            }),
        ),
    };

    Response { id, result, error }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_calculate_offset_newlines() {
        let text = "abc\ndef\nghi";
        assert_eq!(position_to_offset(text, 0, 0), Some(0));
        assert_eq!(position_to_offset(text, 0, 1), Some(1));
        assert_eq!(position_to_offset(text, 0, 2), Some(2));

        assert_eq!(position_to_offset(text, 1, 0), Some(4));
        assert_eq!(position_to_offset(text, 1, 1), Some(5));
        assert_eq!(position_to_offset(text, 1, 2), Some(6));

        assert_eq!(position_to_offset(text, 2, 0), Some(8));
        assert_eq!(position_to_offset(text, 2, 1), Some(9));
        assert_eq!(position_to_offset(text, 2, 2), Some(10));
    }

    #[test]
    fn can_calculate_offset_carriage_returns() {
        let text = "abc\rdef\rghi";
        assert_eq!(position_to_offset(text, 0, 0), Some(0));
        assert_eq!(position_to_offset(text, 0, 1), Some(1));
        assert_eq!(position_to_offset(text, 0, 2), Some(2));

        assert_eq!(position_to_offset(text, 1, 0), Some(4));
        assert_eq!(position_to_offset(text, 1, 1), Some(5));
        assert_eq!(position_to_offset(text, 1, 2), Some(6));

        assert_eq!(position_to_offset(text, 2, 0), Some(8));
        assert_eq!(position_to_offset(text, 2, 1), Some(9));
        assert_eq!(position_to_offset(text, 2, 2), Some(10));

        assert_eq!(position_to_offset(text, 3, 0), None);
    }

    #[test]
    fn can_calculate_offset_crlf() {
        let text = "abc\r\ndef\r\nghi";
        assert_eq!(position_to_offset(text, 0, 0), Some(0));
        assert_eq!(position_to_offset(text, 0, 1), Some(1));
        assert_eq!(position_to_offset(text, 0, 2), Some(2));

        assert_eq!(position_to_offset(text, 1, 0), Some(5));
        assert_eq!(position_to_offset(text, 1, 1), Some(6));
        assert_eq!(position_to_offset(text, 1, 2), Some(7));

        assert_eq!(position_to_offset(text, 2, 0), Some(10));
        assert_eq!(position_to_offset(text, 2, 1), Some(11));
        assert_eq!(position_to_offset(text, 2, 2), Some(12));

        assert_eq!(position_to_offset(text, 3, 0), None);
    }

    #[test]
    fn can_calculate_position() {
        let text = "abc\r\ndef\r\nghi";

        assert_eq!(position_from_offset(text, 0), Position { line: 0, character: 0 });
        assert_eq!(position_from_offset(text, 1), Position { line: 0, character: 1 });
        assert_eq!(position_from_offset(text, 2), Position { line: 0, character: 2 });

        assert_eq!(position_from_offset(text, 5), Position { line: 1, character: 0 });
        assert_eq!(position_from_offset(text, 6), Position { line: 1, character: 1 });
        assert_eq!(position_from_offset(text, 7), Position { line: 1, character: 2 });

        assert_eq!(position_from_offset(text, 10), Position { line: 2, character: 0 });
        assert_eq!(position_from_offset(text, 11), Position { line: 2, character: 1 });
        assert_eq!(position_from_offset(text, 12), Position { line: 2, character: 2 });
    }
}
