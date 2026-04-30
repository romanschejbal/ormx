# Language Server

`ferriorm-lsp` is a Language Server Protocol implementation for `.ferriorm`
schema files. It provides diagnostics, formatting, hover, completion, and
go-to-definition over the standard LSP `stdio` transport.

## Install

```sh
cargo install --path crates/ferriorm-lsp
# or, once published:
# cargo install ferriorm-lsp
```

The binary is named `ferriorm-lsp`. It speaks LSP on stdin/stdout — log
output goes to stderr (set `RUST_LOG=ferriorm_lsp=debug` to enable verbose
tracing).

## VS Code

Install the [`ferriorm` extension](../../editors/vscode) (the source lives
in this repository at `editors/vscode/`). The extension finds `ferriorm-lsp`
on `PATH` automatically. To override the location, set:

```jsonc
// .vscode/settings.json
{
  "ferriorm.lsp.serverPath": "/absolute/path/to/ferriorm-lsp"
}
```

## Other editors

Any LSP-aware editor can drive `ferriorm-lsp`. For Neovim with `nvim-lspconfig`:

```lua
require("lspconfig").ferriorm.setup({
  cmd = { "ferriorm-lsp" },
  filetypes = { "ferriorm" },
  root_dir = require("lspconfig.util").root_pattern("schema.ferriorm", ".git"),
})
```

For Helix, add to `languages.toml`:

```toml
[[language]]
name = "ferriorm"
file-types = ["ferriorm"]
language-servers = ["ferriorm-lsp"]

[language-server.ferriorm-lsp]
command = "ferriorm-lsp"
```

## Features

| Feature                | Notes                                                    |
| ---------------------- | -------------------------------------------------------- |
| Diagnostics            | Parse + validation errors, with source ranges            |
| Formatting             | Full-document rewrite via `ferriorm-fmt`                  |
| Hover                  | Signatures and leading doc comments for models / enums   |
| Completion             | `@`/`@@` attribute names, scalar types, model/enum names |
| Go-to-definition       | From a field's type identifier to the referenced block   |

Document sync is full-text (each change replaces the document); schemas are
small enough that full re-parse on every keystroke is comfortably fast.

## Troubleshooting

- **Server not starting**: confirm `ferriorm-lsp --version` runs in your
  shell; if not, fix `PATH` or set `ferriorm.lsp.serverPath`.
- **No diagnostics**: ensure your file uses the `.ferriorm` extension and
  the language ID is `ferriorm` in your editor.
- **Verbose logs**: launch the editor with `RUST_LOG=ferriorm_lsp=debug`
  set in the environment; tracing output appears in the editor's LSP log.
