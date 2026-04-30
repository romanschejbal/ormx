# Formatting

The `ferriorm format` command rewrites `.ferriorm` files into a canonical
shape: predictable indentation, aligned field columns, and one block per
section. Comments (both standalone `// ...` lines and same-line trailing
`// ...`) are preserved.

## Usage

```sh
# Format the schema configured by --schema (default: schema.ferriorm)
ferriorm format

# Format specific files or directories
ferriorm format db/users.ferriorm db/posts.ferriorm
ferriorm format db/

# Check formatting without writing (CI-friendly: exits 1 if changes are needed)
ferriorm format --check

# Format from stdin to stdout
cat schema.ferriorm | ferriorm format --stdin
```

## What gets canonicalized

- Block order: `datasource` → `generator(s)` → `enum(s)` → `model(s)`, each
  separated by a single blank line.
- Within a model, field columns are aligned to the longest field name and
  longest field type. Block-level attributes (`@@index`, `@@unique`, `@@map`,
  `@@id`) appear after a blank line and are not column-aligned.
- Runs of multiple blank lines are collapsed to a single blank line.
- The file always ends in exactly one trailing newline.

## Comment preservation

- A `// ...` line immediately above a block, field, or block-attribute is
  attached as a *leading* comment to that node and re-emitted in place.
- A `// ...` on the same line as a field or block-attribute is preserved
  as a *trailing* comment after re-formatting that line.
- A `// ...` separated from the next node by a blank line is treated as a
  floating comment and emitted at the end of the enclosing block (or at the
  end of the file).

## Idempotency

`format(format(s)) == format(s)` is guaranteed for any source the parser
accepts. There's no width-based wrapping, so trailing same-line comments
never trigger re-layout. The property is exercised by the test suite
against every snapshot fixture and several whitespace-perturbed variants.

## Editor integration

Use the [VS Code extension](./lsp.md) (or any LSP client connected to
`ferriorm-lsp`) to format on save. Both the CLI and the language server
delegate to the same formatter, so editor results match `ferriorm format`
exactly.
