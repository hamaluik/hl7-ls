use color_eyre::{
    eyre::{Context, ContextCompat},
    Result,
};
use hl7_parser::parse_message_with_lenient_newlines;
use lsp_textdocument::TextDocuments;
use lsp_types::{ExecuteCommandParams, Uri};
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};
use tracing::instrument;

use super::CommandResult;

#[instrument(level = "debug", skip(documents))]
pub fn handle_send_message_command(
    params: ExecuteCommandParams,
    documents: &TextDocuments,
) -> Result<Option<CommandResult>> {
    if params.arguments.len() < 3 || params.arguments.len() > 4 {
        return Err(color_eyre::eyre::eyre!(
            "Expected 3 or 4 arguments for send message command"
        ));
    }

    let uri: Uri = params.arguments[0]
        .as_str()
        .and_then(|s| s.parse().ok())
        .wrap_err("Expected uri as first argument")?;

    let hostname = params.arguments[1]
        .as_str()
        .wrap_err("Expected hostname as second argument")?;

    let port = params.arguments[2]
        .as_u64()
        .wrap_err("Expected port as third argument")?;

    let timeout = params
        .arguments
        .get(3)
        .and_then(|v| v.as_f64())
        .unwrap_or(5.0);

    let text = documents
        .get_document_content(&uri, None)
        .wrap_err_with(|| format!("no document found for uri: {:?}", uri))?;

    let parse_span = tracing::trace_span!("parse message");
    let _parse_span_guard = parse_span.enter();
    let _message = parse_message_with_lenient_newlines(text)
        .wrap_err_with(|| "Failed to parse HL7 message")?;
    drop(_parse_span_guard);

    tracing::trace!(?uri, ?hostname, ?port, "Sending message");
    let response = send_message(hostname, port as u16, text, timeout)
        .wrap_err("Failed to send message")?;
    tracing::trace!(?response, "Received response");

    Ok(Some(CommandResult::ValueResponse { value: serde_json::Value::String(response) }))
}


#[instrument(level = "info", skip(host, port))]
fn send_message(host: &str, port: u16, message: &str, timeout: f64) -> Result<String> {
    let addr = format!("{}:{}", host, port)
        .to_socket_addrs()
        .wrap_err_with(|| format!("Failed to resolve address for {}:{}", host, port))?
        .next()
        .wrap_err_with(|| "No address found")?;

    let framed = format!(
        "\x0B{message}\x1C\r",
        message = message.replace("\r\n", "\r").replace("\n", "\r")
    );
    let frame_bytes = framed.as_bytes();

    let connection_span = tracing::info_span!("TCP connection", host = host, port = port);
    let send_span = tracing::info_span!(parent: &connection_span, "Send message");
    let receive_span = tracing::info_span!(parent: &connection_span, "Receive message");

    let _connection_guard = connection_span.enter();
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs_f64(timeout))
        .wrap_err_with(|| format!("Failed to connect to {}:{}", host, port))?;
    tracing::info!("Connected");
    stream
        .set_read_timeout(Some(Duration::from_secs_f64(timeout)))
        .wrap_err_with(|| format!("Failed to set read timeout for {}:{}", host, port))?;

    let _send_guard = send_span.enter();
    stream
        .write_all(frame_bytes)
        .wrap_err_with(|| format!("Failed to write message to {}:{}", host, port))?;
    drop(_send_guard);

    let _receive_guard = receive_span.enter();
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    read_till_started(&mut stream, timeout).wrap_err_with(|| "Failed to read start of message")?;
    read_till_ended(&mut stream, &mut buf, timeout).wrap_err_with(|| "Failed to read message")?;
    drop(_receive_guard);

    let message = String::from_utf8(buf).wrap_err_with(|| "Failed to parse message as utf8")?;
    Ok(message.replace("\r", "\n"))
}

#[instrument(level = "trace", skip(stream))]
fn read_till_started(stream: &mut TcpStream, timeout: f64) -> Result<()> {
    let start = Instant::now();
    let timeout = Duration::from_secs_f64(timeout);

    loop {
        let mut byte = [0u8; 1];
        stream
            .read_exact(&mut byte)
            .wrap_err_with(|| "Failed to read byte")?;
        if byte[0] == 0x0B {
            break;
        }

        if start.elapsed() > timeout {
            return Err(color_eyre::eyre::eyre!(
                "Timed out waiting for start of message"
            ));
        }
    }
    Ok(())
}

#[instrument(level = "trace", skip(stream, buffer))]
fn read_till_ended(stream: &mut TcpStream, buffer: &mut Vec<u8>, timeout: f64) -> Result<()> {
    let start = Instant::now();
    let timeout = Duration::from_secs_f64(timeout);

    loop {
        let mut buf = [0u8; 256];
        let count = stream
            .read(buf.as_mut_slice())
            .wrap_err_with(|| "Failed to read byte")?;

        if count == 0 {
            return Err(color_eyre::eyre::eyre!(
                "Connection closed before end of message"
            ));
        }

        // search for the [0x1C, 0x0D] sequence
        // if found, return the buffer
        // if not found, append the buffer and continue
        for c in buf.iter().take(count) {
            buffer.push(*c);
            if buffer.len() >= 2 && buffer[buffer.len() - 2..] == [0x1C, 0x0D] {
                // trim the 2 footer bytes off the message
                buffer.truncate(buffer.len() - 2);
                return Ok(());
            }
            if buffer.len() > 65535 {
                return Err(color_eyre::eyre::eyre!("Message too large (> 65535 bytes)"));
            }
        }

        if start.elapsed() > timeout {
            return Err(color_eyre::eyre::eyre!(
                "Timed out waiting for start of message"
            ));
        }
    }
}
