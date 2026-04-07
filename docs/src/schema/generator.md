# Generator

The `generator` block controls where ferriorm places the generated Rust code and how the code generation process behaves.

## Syntax

```prisma
generator client {
  output = "./src/generated"
}
```

## Fields

### `output`

The path (relative to the schema file) where generated code will be written.

| Field | Type | Default |
|---|---|---|
| `output` | `String` | `"./src/generated"` |

If `output` is omitted, the generator defaults to `./src/generated`.

```prisma
// Uses the default output directory
generator client {
}
```

```prisma
// Custom output directory
generator client {
  output = "./src/db"
}
```

## Rules

- The block name (e.g., `client`) is an identifier you choose. It does not affect the generated code.
- The `output` value is a string literal specifying a path relative to the location of the schema file.
- Multiple `generator` blocks are allowed, each with a different output path.

## Example

A typical project layout after generation:

```
my-project/
  schema.ferriorm
  src/
    generated/        <-- output directory
      mod.rs
      user.rs
      post.rs
      ...
    main.rs
```

The generator creates one module file per model, plus a `mod.rs` that re-exports everything. You then include the generated module in your project with:

```rust
mod generated;

use generated::*;
```
