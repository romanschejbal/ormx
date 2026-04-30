//! Conversions between ferriorm byte spans and LSP positions.

use ferriorm_core::ast::Span;
use tower_lsp::lsp_types::{Position, Range};

/// Pre-computed line-start byte offsets for an indexed document.
///
/// LSP positions are line + UTF-16 code-unit offset; we convert by walking
/// the bytes of the surrounding line. Schemas are tiny so the per-byte cost
/// is negligible.
#[derive(Debug, Clone)]
pub struct LineIndex {
    line_starts: Vec<usize>,
    text_len: usize,
}

impl LineIndex {
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self {
            line_starts: starts,
            text_len: text.len(),
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn position_of(&self, text: &str, offset: usize) -> Position {
        let offset = offset.min(self.text_len);
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let line_start = self.line_starts[line];
        let line_text = &text[line_start..offset];
        let character = line_text.encode_utf16().count();
        Position {
            line: line as u32,
            character: character as u32,
        }
    }

    /// Convert a ferriorm byte span into an LSP range.
    #[must_use]
    pub fn range_of(&self, text: &str, span: Span) -> Range {
        Range {
            start: self.position_of(text, span.start),
            end: self.position_of(text, span.end),
        }
    }

    /// End-of-document position.
    #[must_use]
    pub fn end_position(&self, text: &str) -> Position {
        self.position_of(text, text.len())
    }

    /// Convert an LSP `Position` to a byte offset in the document. Saturates
    /// at the end of the line if `character` exceeds the line length.
    #[must_use]
    pub fn offset_of(&self, text: &str, pos: Position) -> usize {
        let line = pos.line as usize;
        if line >= self.line_starts.len() {
            return self.text_len;
        }
        let line_start = self.line_starts[line];
        let line_end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text_len);
        let line_text = &text[line_start..line_end];
        let mut utf16 = 0usize;
        for (byte_idx, ch) in line_text.char_indices() {
            if utf16 >= pos.character as usize {
                return line_start + byte_idx;
            }
            utf16 += ch.len_utf16();
        }
        line_end
    }
}
