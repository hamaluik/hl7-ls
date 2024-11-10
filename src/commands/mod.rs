use color_eyre::Result;
use lsp_textdocument::TextDocuments;
use lsp_types::{ExecuteCommandParams, WorkspaceEdit};
use tracing::instrument;

mod encode_decode_selection;
mod encode_decode_text;
mod generate_control_id;
mod send_message;
mod set_to_now;

pub const CMD_SET_TO_NOW: &str = "hl7.setTimestampToNow";
pub const CMD_SEND_MESSAGE: &str = "hl7.sendMessage";
pub const CMD_GENERATE_CONTROL_ID: &str = "hl7.generateControlId";
pub const CMD_ENCODE_TEXT: &str = "hl7.encodeText";
pub const CMD_DECODE_TEXT: &str = "hl7.decodeText";
pub const CMD_ENCODE_SELECTION: &str = "hl7.encodeSelection";
pub const CMD_DECODE_SELECTION: &str = "hl7.decodeSelection";

pub enum CommandResult {
    WorkspaceEdit {
        label: &'static str,
        edit: WorkspaceEdit,
    },
    ValueResponse {
        value: serde_json::Value,
    },
}

#[instrument(level = "debug", skip(params, documents))]
pub fn handle_execute_command_request(
    params: ExecuteCommandParams,
    documents: &TextDocuments,
) -> Result<Option<CommandResult>> {
    match params.command.as_str() {
        CMD_SET_TO_NOW => set_to_now::handle_set_to_now_command(params, documents),
        CMD_SEND_MESSAGE => send_message::handle_send_message_command(params, documents),
        CMD_GENERATE_CONTROL_ID => {
            generate_control_id::handle_generate_control_id_command(params, documents)
        }
        CMD_ENCODE_TEXT => encode_decode_text::handle_encode_text_command(params, documents),
        CMD_DECODE_TEXT => encode_decode_text::handle_decode_text_command(params, documents),
        CMD_ENCODE_SELECTION => {
            encode_decode_selection::handle_encode_selection_command(params, documents)
        }
        CMD_DECODE_SELECTION => {
            encode_decode_selection::handle_decode_selection_command(params, documents)
        }
        _ => {
            tracing::warn!(command = ?params.command, args = ?params.arguments, "Unknown command");
            Ok(None)
        }
    }
}
