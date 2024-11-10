# hl7-ls

A Language Server for HL7v2 messages implementing [LSP 3.17](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/). Not that useful on its own, best paired with [hl7-ls-nvim](https://github.com/hamaluik/hl7-ls-nvim) or [hl7-ls-vscode](https://github.com/hamaluik/hl7-ls-vscode)

## LSP Features

* Note: hl7-ls _only_ supports `stdio` communications.

### Developed

- Diagnostics
- Hover
- Completion
- Document Symbols
- Code Actions
- Execute Command. Supported commands:
    * `hl7.setTimestampToNow`: Set the timestamp at the current cursor position to the current time
    * `hl7.sendMessage`: Send the current message to the given destination
    * `hl7.generateControlId`: Set MSH.10 to a new random 20-character string
- Selection Range
- Custom field descriptions
- Signature Help

### In Progress

- Custom validations

### Planned

- Semantic Tokens

### Not Planned

- Go to definition, declaration, type definition, implementation, and references
- Document Formatting
- Document Highlighting
- Document Linking
- Rename
- Folding Range
- Code Lens

## Installation

### Prerequisites

- [rust](https://www.rust-lang.org/tools/install)
- [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)

### Install

```bash
cargo install hl7-ls
```

## Usage

```
hl7-ls 1.0.0-pre
by Kenton Hamaluik <kenton@hamaluik.ca>
A Language Server for HL7 messages

Usage: hl7-ls [OPTIONS] [COMMAND]

Commands:
  log-to-stderr  Log outout to standard error (default)
  log-to-file    Log output to a file
  help           Print this message or the help of the given subcommand(s)

Options:
  -c, --colour <COLOUR>
          Control whether color is used in the output

          [default: auto]
          [possible values: auto, always, never]

  -v, --verbose...
          Enable debugging output

          Use multiple times to increase verbosity (e.g., -v, -vv)

      --vscode
          Enable Visual Studio Code mode

          This mode is intended for when running this language server through Visual Studio Code.

      --disable-std-table-validations
          Disable standard table value validation checks

          This will disable table value validation checks for table values that are not defined in the workspace (and come from the HL7 standard).

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

```

## Supported Commands

### Set Timestamp to Now: `hl7.setTimestampToNow`

Set the timestamp at the current cursor position to the current time.

#### Arguments

1. `uri`: The URI of the document to update
2. `range`: The range of the timestamp to update

### Send Message: `hl7.sendMessage`

Send the message to the given destination using unencrypted `mllp`, and return
the response from the destination.

#### Arguments

1. `uri`: The URI of the document to send
2. `hostname`: The hostname of the destination
3. `port`: The port of the destination
4. `timeout` (_optional_): The timeout in seconds to wait for a response

### Generate Control ID: `hl7.generateControlId`

Set MSH.10 to a new random 20-character string.

#### Arguments

1. `uri`: The URI of the document to update

### Encode Text: `hl7.encodeText`

Encode (escape) HL7 characters in the given text. If the uri is provided, the
encoding in the document is used; otherwise the default encoding (`|^~\&`) is
used.

#### Arguments

1. `text`: The text to encode
2. `uri` (_optional_): The URI of the document used to encode with

### Decode Text: `hl7.decodeText`

Decode (unescape) HL7 characters in the given text. If the uri is provided, the
encoding in the document is used; otherwise the default encoding (`|^~\&`) is
used.

#### Arguments

1. `text`: The text to decode
2. `uri` (_optional_): The URI of the document used to decode with

### Encode Selection: `hl7.encodeSelection`

Encode (escape) the selected range using the encoding in the document. Note
that encoding is done in-place, so the range will be replaced with the encoded
text which may cause the range to be invalid.

#### Arguments

1. `uri`: The URI of the document
2. `range`: The range of the text to encode

### Decode Selection: `hl7.decodeSelection`

Decode (unescape) the selected range using the encoding in the document. Note
that decoding is done in-place, so the range will be replaced with the decoded
text which may cause the range to be invalid.

#### Arguments

1. `uri`: The URI of the document
2. `range`: The range of the text to decode

## Custom Validation

Custom validation rules can be added to the workspace configuration files. The
configuration files are [TOML](https://toml.io/en/) files whose names must end
with `.hl7v.toml` and must be located beneath the workspace root directory.

The custom validation rules can add custom descriptions, table values, and set
the `required` flag for segments and fields.

### Schema

```toml
name = "<name of the workspace configuration>"

[[segments]]
name = "<3-character segment name to identify the segment>"
description = "<optional description of the segment>"

[segments.fields.<field number>]
description = "<optional description of the field>"
required = true # optional, defaults to false
datatype = "<optional HL7 datatype of the field>"
allowed_values = [["<table value 1>", "<description>"], ["<table value 2>", "<description>"], ...]
```

### Example

```toml
# example_workspace.hl7v.toml
name = "Example Workspace"

[[segments]]
name = "PID"

[segments.fields.3]
description = "Medical Record Number (MRN)"
[segments.fields.4]
description = "Enterprise ID (EID)"
required = true

[[segments]]
name = "PV1"

[segments.fields.2]
description = "Patient Class"
required = true
allowed_values = [["I", "Inpatient"], ["O", "Outpatient"]] # note: the spec specifies other values, but our workspace only allows I or O
```

