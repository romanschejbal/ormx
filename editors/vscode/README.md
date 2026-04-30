# ferriorm — VS Code extension

Syntax highlighting and Language Server Protocol support for `.ferriorm`
schema files.

## Features

- Syntax highlighting (TextMate grammar)
- Diagnostics (parse + validation errors) via `ferriorm-lsp`
- Document formatting via `ferriorm-lsp`
- Hover for model and enum definitions
- Completion for `@`/`@@` attributes and types
- Go-to-definition for model and enum references

## Requirements

The extension expects the `ferriorm-lsp` binary to be available. Install it
from the workspace:

```sh
cargo install --path crates/ferriorm-lsp
# or, once published:
# cargo install ferriorm-lsp
```

If `ferriorm-lsp` is not on your `PATH`, set `ferriorm.lsp.serverPath` in
your VS Code settings.

## Building from source

```sh
cd editors/vscode
npm install
npm run compile
```

To produce a `.vsix`:

```sh
npm run package
```

## License

MIT OR Apache-2.0
