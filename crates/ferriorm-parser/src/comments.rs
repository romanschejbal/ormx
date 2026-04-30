//! Comment scanner and attachment.
//!
//! The PEG grammar (`grammar.pest`) treats `// ...` comments as silent tokens
//! so they don't pollute the AST. To support comment-preserving formatting,
//! this module re-scans the source for comments after parsing and attaches
//! them to AST nodes by source position.
//!
//! Attachment rules (mirrors Prisma's formatter):
//!
//! - A non-standalone `// ...` whose start lies within a leaf node's span
//!   (`FieldDef`, `BlockAttrEntry`) is that node's `trailing` comment. Pest
//!   extends a rule's span to consume implicit trailing whitespace and
//!   comments, so `c.span.start ∈ [node.span.start, node.span.end)` is the
//!   right test.
//! - A run of standalone `// ...` lines whose lines are exactly
//!   `node_line - 1`, `node_line - 2`, ... (no blank line between them and
//!   the node) become that node's `leading` comments.
//! - Standalone comments that don't form a contiguous chain back to a node
//!   (i.e. there's a blank line between them and the next node) become
//!   floating comments and are surfaced via [`CommentAttacher::drain_floating_in`]
//!   or [`CommentAttacher::drain_remaining`] for the enclosing block.

use ferriorm_core::ast::{Comments, Span};

/// A `// ...` comment scanned from the source.
#[derive(Debug, Clone)]
pub struct RawComment {
    /// Byte span of the `//` through the last char before the newline.
    pub span: Span,
    /// 1-indexed line of the comment.
    pub line: usize,
    /// Comment text *including* the leading `//`.
    pub text: String,
    /// True when the line contains only whitespace before the `//`.
    pub standalone: bool,
}

/// Scan `source` for all `// ...` comments. Comments inside string literals
/// are correctly ignored.
///
/// # Panics
///
/// Panics if a comment's byte range contains invalid UTF-8 (impossible for
/// strings created from valid `&str` input — kept as an internal invariant
/// check).
#[must_use]
pub fn scan(source: &str) -> Vec<RawComment> {
    let bytes = source.as_bytes();
    let mut comments = Vec::new();
    let mut i = 0;
    let mut line = 1usize;
    let mut in_string = false;
    let mut line_started_with_only_ws = true;

    while i < bytes.len() {
        let b = bytes[i];

        if in_string {
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            if b == b'\n' {
                line += 1;
                line_started_with_only_ws = true;
            }
            i += 1;
            continue;
        }

        match b {
            b'\n' => {
                line += 1;
                line_started_with_only_ws = true;
                i += 1;
            }
            b'"' => {
                in_string = true;
                line_started_with_only_ws = false;
                i += 1;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                let start = i;
                let standalone = line_started_with_only_ws;
                let comment_line = line;
                let mut end = i + 2;
                while end < bytes.len() && bytes[end] != b'\n' {
                    end += 1;
                }
                let text = std::str::from_utf8(&bytes[start..end])
                    .expect("source is valid utf8")
                    .to_string();
                comments.push(RawComment {
                    span: Span { start, end },
                    line: comment_line,
                    text,
                    standalone,
                });
                i = end;
                line_started_with_only_ws = false;
            }
            b' ' | b'\t' | b'\r' => {
                i += 1;
            }
            _ => {
                line_started_with_only_ws = false;
                i += 1;
            }
        }
    }

    comments
}

fn build_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn offset_to_line(line_starts: &[usize], offset: usize) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    }
}

fn to_text(raw: &RawComment) -> String {
    let body = raw.text.trim_start_matches('/');
    body.strip_prefix(' ').unwrap_or(body).to_string()
}

/// Holds scanned comments and lets the parser claim them as it walks the AST.
///
/// Each comment is consumed exactly once. Comments that aren't claimed by any
/// node end up in floating drain calls.
pub struct CommentAttacher {
    line_starts: Vec<usize>,
    comments: Vec<RawComment>,
    /// True once a comment has been consumed (claimed by a node).
    consumed: Vec<bool>,
}

impl CommentAttacher {
    #[must_use]
    pub fn new(source: &str) -> Self {
        let comments = scan(source);
        let consumed = vec![false; comments.len()];
        Self {
            line_starts: build_line_starts(source),
            comments,
            consumed,
        }
    }

    fn line_of(&self, offset: usize) -> usize {
        offset_to_line(&self.line_starts, offset)
    }

    /// Claim the run of standalone comments that ends on the line directly
    /// before `node_start` and extends contiguously upward (no blank lines).
    pub fn take_leading_for(&mut self, node_start: usize) -> Vec<String> {
        let node_line = self.line_of(node_start);
        let mut chain: Vec<usize> = Vec::new();
        let mut expected_line = node_line.saturating_sub(1);
        if expected_line == 0 {
            return Vec::new();
        }

        // Walk the comments from highest position downwards, picking up
        // standalone comments that are unconsumed and on `expected_line`.
        for i in (0..self.comments.len()).rev() {
            if self.consumed[i] {
                continue;
            }
            let c = &self.comments[i];
            // Skip comments that are after this node entirely.
            if c.span.start >= node_start {
                continue;
            }
            if !c.standalone {
                break;
            }
            if c.line != expected_line {
                break;
            }
            chain.push(i);
            expected_line = expected_line.saturating_sub(1);
            if expected_line == 0 {
                break;
            }
        }

        chain.reverse();
        chain
            .into_iter()
            .map(|i| {
                self.consumed[i] = true;
                to_text(&self.comments[i])
            })
            .collect()
    }

    /// Claim the inline trailing comment for a leaf node. Pest extends the
    /// node's rule span to consume the trailing implicit comment, so the
    /// comment's `span.start` lies inside `[node_span.start, node_span.end)`.
    pub fn take_trailing_within(&mut self, span: Span) -> Option<String> {
        let idx = self.comments.iter().enumerate().find_map(|(i, c)| {
            if !self.consumed[i]
                && !c.standalone
                && c.span.start >= span.start
                && c.span.start < span.end
            {
                Some(i)
            } else {
                None
            }
        })?;
        self.consumed[idx] = true;
        Some(to_text(&self.comments[idx]))
    }

    /// Build a [`Comments`] for a leaf node (consumes both leading and
    /// trailing).
    pub fn leaf_comments(&mut self, span: Span) -> Comments {
        let leading = self.take_leading_for(span.start);
        let trailing = self.take_trailing_within(span);
        Comments { leading, trailing }
    }

    /// Build a [`Comments`] for a container node. Captures leading
    /// comments only; inner inline comments are left for child nodes to
    /// claim or are surfaced as floating comments.
    pub fn container_leading(&mut self, span: Span) -> Comments {
        let leading = self.take_leading_for(span.start);
        Comments {
            leading,
            trailing: None,
        }
    }

    /// Drain unclaimed standalone comments whose start offset is in
    /// `[from, before)`. Used to surface comments that appear inside a
    /// block body but don't precede any further node.
    pub fn drain_floating_in(&mut self, from: usize, before: usize) -> Vec<String> {
        let mut out = Vec::new();
        for (i, c) in self.comments.iter().enumerate() {
            if self.consumed[i] {
                continue;
            }
            if c.span.start < from || c.span.start >= before {
                continue;
            }
            if c.standalone {
                out.push(to_text(c));
            }
            self.consumed[i] = true;
        }
        out
    }

    /// Drain everything still unclaimed (standalone only). Used at end of
    /// file.
    pub fn drain_remaining(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for (i, c) in self.comments.iter().enumerate() {
            if self.consumed[i] {
                continue;
            }
            if c.standalone {
                out.push(to_text(c));
            }
            self.consumed[i] = true;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_simple_comments() {
        let src = "// foo\nmodel X { id String @id } // trailing\n// end\n";
        let comments = scan(src);
        assert_eq!(comments.len(), 3);
        assert!(comments[0].standalone);
        assert!(!comments[1].standalone);
        assert!(comments[2].standalone);
        assert_eq!(comments[0].text, "// foo");
        assert_eq!(comments[1].text, "// trailing");
    }

    #[test]
    fn ignores_double_slash_inside_strings() {
        let src = r#"datasource db { url = "postgres://x" } // trailing"#;
        let comments = scan(src);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "// trailing");
    }
}
