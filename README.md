# hl7-ls

A Language Server for HL7v2 messages implementing [LSP 3.17](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/). Not that useful on its own, best paired with [hl7-ls-nvim](https://github.com/hamaluik/hl7-ls-nvim) or [hl7-ls-vscode](https://github.com/hamaluik/hl7-ls-vscode)

## Features

* Note: hl7-ls _only_ supports `stdio` communications.

### Developed

- Diagnostics
- Hover
- Completion
- Document Symbols
- Code Actions
- Code Lens
- Execute Command

### Planned

- Selection Range
- Semantic Tokens
- Signature Help

#### Extensions

- Send HL7 message and receive response

### Not Planned

- Go to definition, declaration, type definition, implementation, and references
- Document Formatting
- Document Highlighting
- Document Linking
- Rename
- Folding Range

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
$ ./hl7-ls --help
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

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
