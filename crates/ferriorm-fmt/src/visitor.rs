//! AST → canonical-string formatter.

use ferriorm_core::ast::{
    BlockAttrEntry, BlockAttribute, Comments, Datasource, DefaultValue, EnumDef, FieldAttribute,
    FieldDef, Generator, IndexAttribute, LiteralValue, ModelDef, ReferentialAction,
    RelationAttribute, SchemaFile, StringOrEnv,
};

use crate::writer::{INDENT, Writer};

#[must_use]
pub fn format_schema_file(file: &SchemaFile) -> String {
    let mut w = Writer::new();
    let mut first = true;

    if let Some(ds) = &file.datasource {
        format_datasource(&mut w, ds);
        first = false;
    }
    for g in &file.generators {
        if !first {
            w.blank_line();
        }
        format_generator(&mut w, g);
        first = false;
    }
    for e in &file.enums {
        if !first {
            w.blank_line();
        }
        format_enum(&mut w, e);
        first = false;
    }
    for m in &file.models {
        if !first {
            w.blank_line();
        }
        format_model(&mut w, m);
        first = false;
    }
    if !file.trailing_comments.is_empty() {
        if !first {
            w.blank_line();
        }
        for c in &file.trailing_comments {
            w.line(&format!("// {c}"));
        }
    }

    w.into_string()
}

fn write_leading(w: &mut Writer, indent: usize, comments: &Comments) {
    for line in &comments.leading {
        for _ in 0..indent {
            w.push_str(INDENT);
        }
        if line.is_empty() {
            w.line("//");
        } else {
            w.line(&format!("// {line}"));
        }
    }
}

fn maybe_trailing(comments: &Comments) -> String {
    match &comments.trailing {
        Some(t) if t.is_empty() => " //".to_string(),
        Some(t) => format!(" // {t}"),
        None => String::new(),
    }
}

fn format_datasource(w: &mut Writer, ds: &Datasource) {
    write_leading(w, 0, &ds.comments);
    w.line(&format!("datasource {} {{", ds.name));
    let kvs: Vec<(&str, String)> = vec![
        ("provider", format!("\"{}\"", ds.provider)),
        ("url", format_string_or_env(&ds.url)),
    ];
    write_aligned_kvs(w, &kvs);
    w.line("}");
}

fn format_generator(w: &mut Writer, g: &Generator) {
    write_leading(w, 0, &g.comments);
    w.line(&format!("generator {} {{", g.name));
    if let Some(out) = &g.output {
        let kvs: Vec<(&str, String)> = vec![("output", format!("\"{out}\""))];
        write_aligned_kvs(w, &kvs);
    }
    w.line("}");
}

fn write_aligned_kvs(w: &mut Writer, kvs: &[(&str, String)]) {
    let max_key = kvs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in kvs {
        let pad = " ".repeat(max_key - k.len());
        w.push_str(INDENT);
        w.push_str(k);
        w.push_str(&pad);
        w.push_str(" = ");
        w.line(v);
    }
}

fn format_string_or_env(soe: &StringOrEnv) -> String {
    match soe {
        StringOrEnv::Literal(s) => format!("\"{s}\""),
        StringOrEnv::Env(var) => format!("env(\"{var}\")"),
    }
}

fn format_enum(w: &mut Writer, e: &EnumDef) {
    write_leading(w, 0, &e.comments);
    w.line(&format!("enum {} {{", e.name));
    for variant in &e.variants {
        w.push_str(INDENT);
        w.line(variant);
    }
    if let Some(map) = &e.db_name {
        w.blank_line();
        w.push_str(INDENT);
        w.line(&format!("@@map(\"{map}\")"));
    }
    w.line("}");
}

fn format_model(w: &mut Writer, m: &ModelDef) {
    write_leading(w, 0, &m.comments);
    w.line(&format!("model {} {{", m.name));

    // Pre-render each field's three columns so we can align across all
    // fields in this model.
    let rendered: Vec<RenderedField> = m
        .fields
        .iter()
        .map(|f| RenderedField {
            comments: &f.comments,
            name: f.name.clone(),
            type_repr: render_field_type(f),
            attrs_repr: render_field_attrs(&f.attributes),
        })
        .collect();

    let max_name = rendered.iter().map(|r| r.name.len()).max().unwrap_or(0);
    let max_type = rendered
        .iter()
        .map(|r| r.type_repr.len())
        .max()
        .unwrap_or(0);

    for r in &rendered {
        write_leading(w, 1, r.comments);
        w.push_str(INDENT);
        w.push_str(&r.name);
        w.push_str(&" ".repeat(max_name - r.name.len()));
        w.push(' ');
        w.push_str(&r.type_repr);
        if !r.attrs_repr.is_empty() {
            w.push_str(&" ".repeat(max_type - r.type_repr.len()));
            w.push(' ');
            w.push_str(&r.attrs_repr);
        }
        w.push_str(&maybe_trailing(r.comments));
        w.push('\n');
    }

    if !m.attributes.is_empty() {
        w.blank_line();
        for attr in &m.attributes {
            format_block_attr(w, attr);
        }
    }

    if !m.trailing_comments.is_empty() {
        w.blank_line();
        for c in &m.trailing_comments {
            w.push_str(INDENT);
            if c.is_empty() {
                w.line("//");
            } else {
                w.line(&format!("// {c}"));
            }
        }
    }

    w.line("}");
}

struct RenderedField<'a> {
    comments: &'a Comments,
    name: String,
    type_repr: String,
    attrs_repr: String,
}

fn render_field_type(f: &FieldDef) -> String {
    let mut s = f.field_type.name.clone();
    if f.field_type.is_list {
        s.push_str("[]");
    } else if f.field_type.is_optional {
        s.push('?');
    }
    s
}

fn render_field_attrs(attrs: &[FieldAttribute]) -> String {
    attrs
        .iter()
        .map(render_field_attr)
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_field_attr(attr: &FieldAttribute) -> String {
    match attr {
        FieldAttribute::Id => "@id".to_string(),
        FieldAttribute::Unique => "@unique".to_string(),
        FieldAttribute::UpdatedAt => "@updatedAt".to_string(),
        FieldAttribute::Default(d) => format!("@default({})", render_default(d)),
        FieldAttribute::Map(name) => format!("@map(\"{name}\")"),
        FieldAttribute::Relation(r) => format!("@relation({})", render_relation(r)),
        FieldAttribute::DbType(name, args) => {
            if args.is_empty() {
                format!("@db.{name}")
            } else {
                let inner = args
                    .iter()
                    .map(|a| {
                        // The parser stores all `@db.X(...)` args as strings,
                        // including unquoted integers/identifiers. Heuristic:
                        // emit quotes only when the value isn't pure digits.
                        if a.chars().all(|c| c.is_ascii_digit()) {
                            a.clone()
                        } else {
                            format!("\"{a}\"")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("@db.{name}({inner})")
            }
        }
    }
}

fn render_default(d: &DefaultValue) -> String {
    match d {
        DefaultValue::Uuid => "uuid()".to_string(),
        DefaultValue::Cuid => "cuid()".to_string(),
        DefaultValue::AutoIncrement => "autoincrement()".to_string(),
        DefaultValue::Now => "now()".to_string(),
        DefaultValue::Literal(LiteralValue::String(s)) => format!("\"{s}\""),
        DefaultValue::Literal(LiteralValue::Int(i)) => i.to_string(),
        DefaultValue::Literal(LiteralValue::Float(fl)) => format!("{fl}"),
        DefaultValue::Literal(LiteralValue::Bool(b)) => b.to_string(),
        DefaultValue::EnumVariant(v) => v.clone(),
    }
}

fn render_relation(r: &RelationAttribute) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(name) = &r.name {
        parts.push(format!("\"{name}\""));
    }
    if !r.fields.is_empty() {
        parts.push(format!("fields: [{}]", r.fields.join(", ")));
    }
    if !r.references.is_empty() {
        parts.push(format!("references: [{}]", r.references.join(", ")));
    }
    if let Some(action) = r.on_delete {
        parts.push(format!("onDelete: {}", render_action(action)));
    }
    if let Some(action) = r.on_update {
        parts.push(format!("onUpdate: {}", render_action(action)));
    }
    parts.join(", ")
}

fn render_action(a: ReferentialAction) -> &'static str {
    match a {
        ReferentialAction::Cascade => "Cascade",
        ReferentialAction::Restrict => "Restrict",
        ReferentialAction::NoAction => "NoAction",
        ReferentialAction::SetNull => "SetNull",
        ReferentialAction::SetDefault => "SetDefault",
    }
}

fn format_block_attr(w: &mut Writer, entry: &BlockAttrEntry) {
    write_leading(w, 1, &entry.comments);
    w.push_str(INDENT);
    let body = match &entry.kind {
        BlockAttribute::Index(idx) => format!("@@index({})", render_index_args(idx)),
        BlockAttribute::Unique(idx) => format!("@@unique({})", render_index_args(idx)),
        BlockAttribute::Map(name) => format!("@@map(\"{name}\")"),
        BlockAttribute::Id(fields) => format!("@@id([{}])", fields.join(", ")),
    };
    w.push_str(&body);
    w.push_str(&maybe_trailing(&entry.comments));
    w.push('\n');
}

fn render_index_args(idx: &IndexAttribute) -> String {
    let fields = format!("[{}]", idx.fields.join(", "));
    match &idx.name {
        Some(n) => format!("{fields}, name: \"{n}\""),
        None => fields,
    }
}
