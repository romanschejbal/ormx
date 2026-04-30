//! Tiny string-builder used by the formatter.

/// Indent unit (two ASCII spaces). Hardcoded because the formatter is opinionated.
pub const INDENT: &str = "  ";

/// Append-only string buffer with a few formatter-specific helpers.
#[derive(Default)]
pub struct Writer {
    buf: String,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_str(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    pub fn push(&mut self, c: char) {
        self.buf.push(c);
    }

    /// Write a single line followed by a newline.
    pub fn line(&mut self, s: &str) {
        self.buf.push_str(s);
        self.buf.push('\n');
    }

    /// Write a blank line. Idempotent: collapses to at most one consecutive
    /// blank line in the output.
    pub fn blank_line(&mut self) {
        if self.buf.ends_with("\n\n") || self.buf.is_empty() {
            return;
        }
        if !self.buf.ends_with('\n') {
            self.buf.push('\n');
        }
        self.buf.push('\n');
    }

    /// Trim trailing blank lines, then ensure exactly one trailing newline.
    pub fn into_string(mut self) -> String {
        while self.buf.ends_with("\n\n") {
            self.buf.pop();
        }
        if !self.buf.ends_with('\n') {
            self.buf.push('\n');
        }
        self.buf
    }
}
