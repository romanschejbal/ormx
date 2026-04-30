//! Semantic validation of the raw AST into the resolved Schema IR.
//!
//! The validator walks the [`ferriorm_core::ast::SchemaFile`] produced by the
//! parser and performs the following:
//!
//! - Resolves field type names to scalars, enums, or model references.
//! - Infers database table and column names from `@@map`/`@map` or `snake_case`
//!   conventions.
//! - Checks that every model has a primary key (`@id` or `@@id`).
//! - Detects duplicate model/enum names and unknown type references.
//! - Resolves relation cardinality and referential actions.
//!
//! The output is an [`ferriorm_core::schema::Schema`], the canonical IR consumed
//! by codegen and the migration engine.

use std::collections::HashSet;

use ferriorm_core::ast;
use ferriorm_core::error::CoreError;
use ferriorm_core::schema::{
    DatasourceConfig, Enum, Field, FieldKind, GeneratorConfig, Index, Model, PrimaryKey,
    RelationType, ResolvedRelation, Schema, UniqueConstraint,
};
use ferriorm_core::types::{DatabaseProvider, ScalarType};
use ferriorm_core::utils::to_snake_case;

/// Validate a parsed AST and produce a resolved Schema IR.
///
/// # Errors
///
/// Returns a [`CoreError`] if the AST has validation problems such as
/// missing primary keys, unknown types, or duplicate names.
pub fn validate(ast: &ast::SchemaFile) -> Result<Schema, CoreError> {
    let datasource = validate_datasource(ast)?;
    let generators = validate_generators(ast)?;
    let enums = validate_enums(ast)?;
    let models = validate_models(ast, &enums)?;

    validate_unique_db_names(&models, ast)?;
    validate_relation_disambiguation(&models, ast)?;

    Ok(Schema {
        datasource,
        generators,
        enums,
        models,
    })
}

/// Two models cannot map to the same database table (`@@map("..."` /
/// implicit snake_case-plural). Catching this here prevents conflicting
/// CREATE TABLE statements at migration time.
fn validate_unique_db_names(models: &[Model], ast: &ast::SchemaFile) -> Result<(), CoreError> {
    use std::collections::HashMap;
    let mut seen: HashMap<&str, &str> = HashMap::new();
    for m in models {
        if let Some(existing) = seen.get(m.db_name.as_str()) {
            return Err(CoreError::Validation {
                message: format!(
                    "Duplicate table name `{}` (used by models `{}` and `{}`). \
                     Each model must map to a distinct table; use `@@map(\"...\")` to disambiguate.",
                    m.db_name, existing, m.name,
                ),
                span: model_span(ast, &m.name),
            });
        }
        seen.insert(&m.db_name, &m.name);
    }
    Ok(())
}

/// Is `s` a Rust keyword (reserved or strict)? Used to reject schema
/// field names that would cause `format_ident!` to panic in codegen.
fn is_rust_keyword(s: &str) -> bool {
    matches!(
        s,
        // Strict keywords
        "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern"
        | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match"
        | "mod" | "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self"
        | "static" | "struct" | "super" | "trait" | "true" | "type" | "unsafe"
        | "use" | "where" | "while"
        // 2018+ keywords
        | "async" | "await" | "dyn"
        // Reserved (might become keywords)
        | "abstract" | "become" | "box" | "do" | "final" | "macro" | "override"
        | "priv" | "typeof" | "unsized" | "virtual" | "yield" | "try"
    )
}

/// Look up the AST `ModelDef` span by name. Used to attach source spans to
/// validation errors that were detected from the post-validate IR.
fn model_span(ast: &ast::SchemaFile, name: &str) -> Option<ast::Span> {
    ast.models.iter().find(|m| m.name == name).map(|m| m.span)
}

/// Look up the AST `FieldDef` span by model + field name.
fn field_span(ast: &ast::SchemaFile, model_name: &str, field_name: &str) -> Option<ast::Span> {
    ast.models
        .iter()
        .find(|m| m.name == model_name)
        .and_then(|m| m.fields.iter().find(|f| f.name == field_name))
        .map(|f| f.span)
}

/// When two or more fields on the same model are *forward* FKs to the
/// same target, OR two or more are *back-references* (implicit lists
/// or `@relation` without `fields:`) from the same target, each must
/// use `@relation("Name", ...)` to disambiguate. The forward and
/// back-reference sides are tracked separately so a `parent` + `children`
/// self-reference (one forward, one back) is unambiguous.
fn validate_relation_disambiguation(
    models: &[Model],
    ast: &ast::SchemaFile,
) -> Result<(), CoreError> {
    use std::collections::{HashMap, HashSet};

    for model in models {
        // (target_model_name, is_fk_owner) -> fields in that group.
        let mut groups: HashMap<(&str, bool), Vec<&Field>> = HashMap::new();

        for field in &model.fields {
            let target = match &field.field_type {
                FieldKind::Model(name) => name.as_str(),
                _ => continue,
            };
            let is_fk_owner = field
                .relation
                .as_ref()
                .is_some_and(|r| !r.fields.is_empty());
            groups.entry((target, is_fk_owner)).or_default().push(field);
        }

        for ((target, _), group) in &groups {
            if group.len() < 2 {
                continue;
            }

            let mut seen_names: HashSet<&str> = HashSet::new();
            for field in group {
                let name = field.relation.as_ref().and_then(|r| r.name.as_deref());
                let Some(n) = name else {
                    return Err(CoreError::Validation {
                        message: format!(
                            "Multiple relations from `{}` to `{}` require disambiguation. \
                             Add `@relation(\"<Name>\", ...)` to each related field on both sides.",
                            model.name, target,
                        ),
                        span: field_span(ast, &model.name, &field.name),
                    });
                };
                if !seen_names.insert(n) {
                    return Err(CoreError::Validation {
                        message: format!(
                            "Duplicate relation name `{}` between `{}` and `{}`. \
                             Each relation between the same pair of models must have a unique name.",
                            n, model.name, target,
                        ),
                        span: field_span(ast, &model.name, &field.name),
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_datasource(ast: &ast::SchemaFile) -> Result<DatasourceConfig, CoreError> {
    let ds = ast.datasource.as_ref().ok_or(CoreError::Validation {
        message: "Missing datasource block".into(),
        span: None,
    })?;

    let provider =
        ds.provider
            .parse::<DatabaseProvider>()
            .map_err(|_| CoreError::UnknownProvider {
                provider: ds.provider.clone(),
                span: Some(ds.span),
            })?;

    let url = match &ds.url {
        ast::StringOrEnv::Literal(s) => s.clone(),
        ast::StringOrEnv::Env(var) => format!("${{env:{var}}}"),
    };

    Ok(DatasourceConfig {
        name: ds.name.clone(),
        provider,
        url,
    })
}

fn validate_generators(ast: &ast::SchemaFile) -> Result<Vec<GeneratorConfig>, CoreError> {
    ast.generators
        .iter()
        .map(|g| {
            Ok(GeneratorConfig {
                name: g.name.clone(),
                output: g.output.clone().unwrap_or_else(|| "./src/generated".into()),
            })
        })
        .collect()
}

fn validate_enums(ast: &ast::SchemaFile) -> Result<Vec<Enum>, CoreError> {
    let mut names = HashSet::new();
    let mut result = Vec::new();

    for e in &ast.enums {
        if !names.insert(&e.name) {
            return Err(CoreError::DuplicateName {
                name: e.name.clone(),
                kind: "enum",
                span: Some(e.span),
            });
        }

        result.push(Enum {
            name: e.name.clone(),
            db_name: e.db_name.clone().unwrap_or_else(|| to_snake_case(&e.name)),
            variants: e.variants.clone(),
        });
    }

    Ok(result)
}

fn validate_models(ast: &ast::SchemaFile, enums: &[Enum]) -> Result<Vec<Model>, CoreError> {
    let enum_names: HashSet<&str> = enums.iter().map(|e| e.name.as_str()).collect();
    let model_names: HashSet<&str> = ast.models.iter().map(|m| m.name.as_str()).collect();
    let mut seen_names = HashSet::new();

    let mut result = Vec::new();

    for model_def in &ast.models {
        if !seen_names.insert(&model_def.name) {
            return Err(CoreError::DuplicateName {
                name: model_def.name.clone(),
                kind: "model",
                span: Some(model_def.span),
            });
        }

        // Check for name collision with enums
        if enum_names.contains(model_def.name.as_str()) {
            return Err(CoreError::DuplicateName {
                name: model_def.name.clone(),
                kind: "model/enum",
                span: Some(model_def.span),
            });
        }

        let model = validate_model(model_def, &enum_names, &model_names)?;
        result.push(model);
    }

    Ok(result)
}

#[allow(clippy::too_many_lines)] // sequential validation passes; splitting hides the order
fn validate_model(
    model_def: &ast::ModelDef,
    enum_names: &HashSet<&str>,
    model_names: &HashSet<&str>,
) -> Result<Model, CoreError> {
    // Resolve @@map
    let db_name = model_def
        .attributes
        .iter()
        .find_map(|a| match &a.kind {
            ast::BlockAttribute::Map(name) => Some(name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| to_snake_case(&model_def.name) + "s");

    let mut fields = Vec::new();
    let mut has_id_field = false;

    for field_def in &model_def.fields {
        let field = validate_field(field_def, &model_def.name, enum_names, model_names)?;
        if field.is_id {
            has_id_field = true;
        }
        fields.push(field);
    }

    // Check @@id for composite primary key
    let composite_id: Option<(Vec<String>, ast::Span)> =
        model_def.attributes.iter().find_map(|a| match &a.kind {
            ast::BlockAttribute::Id(fields) => Some((fields.clone(), a.span)),
            _ => None,
        });

    if !has_id_field && composite_id.is_none() {
        return Err(CoreError::MissingPrimaryKey {
            model_name: model_def.name.clone(),
            span: Some(model_def.span),
        });
    }

    // Field-name set for B4 (block-attribute field-existence checks).
    // We accept either the schema field name or the snake_case form.
    let field_name_set: HashSet<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    let field_db_set: HashSet<&str> = fields.iter().map(|f| f.db_name.as_str()).collect();
    let field_resolver = |needle: &str| -> Option<&Field> {
        fields
            .iter()
            .find(|f| f.name == needle || f.db_name == needle || to_snake_case(&f.name) == needle)
    };

    let primary_key = if let Some((composite_fields, attr_span)) = composite_id {
        // B4 (PK): all named fields must exist on the model.
        // B7: PK fields cannot be Json (uncomparable / unhashable in DBs).
        for f in &composite_fields {
            let Some(resolved) = field_resolver(f) else {
                return Err(CoreError::Validation {
                    message: format!(
                        "`@@id` on model `{}` references unknown field `{}`",
                        model_def.name, f,
                    ),
                    span: Some(attr_span),
                });
            };
            if matches!(resolved.field_type, FieldKind::Scalar(ScalarType::Json)) {
                return Err(CoreError::Validation {
                    message: format!(
                        "Field `{}.{}` of type `Json` cannot be part of a composite primary key.",
                        model_def.name, resolved.name,
                    ),
                    span: Some(attr_span),
                });
            }
        }
        PrimaryKey {
            fields: composite_fields,
        }
    } else {
        let id_fields: Vec<String> = fields
            .iter()
            .filter(|f| f.is_id)
            .map(|f| f.name.clone())
            .collect();
        PrimaryKey { fields: id_fields }
    };

    // B4: @@index / @@unique field-existence checks. Each named field
    // must exist on the model; otherwise the migration would emit a
    // CREATE INDEX referencing a non-existent column.
    for attr in &model_def.attributes {
        let (kind, fs) = match &attr.kind {
            ast::BlockAttribute::Index(idx) => ("@@index", &idx.fields),
            ast::BlockAttribute::Unique(idx) => ("@@unique", &idx.fields),
            _ => continue,
        };
        for f in fs {
            if !field_name_set.contains(f.as_str())
                && !field_db_set.contains(f.as_str())
                && field_resolver(f).is_none()
            {
                return Err(CoreError::Validation {
                    message: format!(
                        "`{}` on model `{}` references unknown field `{}`",
                        kind, model_def.name, f,
                    ),
                    span: Some(attr.span),
                });
            }
        }
    }

    // Indexes
    let indexes = model_def
        .attributes
        .iter()
        .filter_map(|a| match &a.kind {
            ast::BlockAttribute::Index(idx) => Some(Index {
                fields: idx.fields.clone(),
                name: idx.name.clone(),
            }),
            _ => None,
        })
        .collect();

    // Unique constraints (from @@unique)
    let unique_constraints = model_def
        .attributes
        .iter()
        .filter_map(|a| match &a.kind {
            ast::BlockAttribute::Unique(idx) => Some(UniqueConstraint {
                fields: idx.fields.clone(),
                name: idx.name.clone(),
            }),
            _ => None,
        })
        .collect();

    Ok(Model {
        name: model_def.name.clone(),
        db_name,
        fields,
        primary_key,
        indexes,
        unique_constraints,
    })
}

#[allow(clippy::too_many_lines)] // sequential per-field checks; splitting hides the order
fn validate_field(
    field_def: &ast::FieldDef,
    model_name: &str,
    enum_names: &HashSet<&str>,
    model_names: &HashSet<&str>,
) -> Result<Field, CoreError> {
    let type_name = &field_def.field_type.name;

    // B1: reject Rust keywords as field names. Codegen would otherwise
    // panic in `format_ident!`. Suggest `@map` for users who need the
    // database column to keep that name.
    if is_rust_keyword(&field_def.name) {
        return Err(CoreError::Validation {
            message: format!(
                "Field name `{}.{}` is a Rust keyword and cannot be used as a struct field. \
                 Rename the field and use `@map(\"{}\")` if you need that database column name.",
                model_name, field_def.name, field_def.name,
            ),
            span: Some(field_def.span),
        });
    }

    let field_type = if let Ok(scalar) = type_name.parse::<ScalarType>() {
        FieldKind::Scalar(scalar)
    } else if enum_names.contains(type_name.as_str()) {
        FieldKind::Enum(type_name.clone())
    } else if model_names.contains(type_name.as_str()) {
        FieldKind::Model(type_name.clone())
    } else {
        return Err(CoreError::UnknownType {
            model_name: model_name.to_string(),
            field_name: field_def.name.clone(),
            type_name: type_name.clone(),
            span: Some(field_def.span),
        });
    };

    let is_id = field_def
        .attributes
        .iter()
        .any(|a| matches!(a, ast::FieldAttribute::Id));
    let is_unique = field_def
        .attributes
        .iter()
        .any(|a| matches!(a, ast::FieldAttribute::Unique));
    let is_updated_at = field_def
        .attributes
        .iter()
        .any(|a| matches!(a, ast::FieldAttribute::UpdatedAt));
    let default = field_def.attributes.iter().find_map(|a| match a {
        ast::FieldAttribute::Default(d) => Some(d.clone()),
        _ => None,
    });

    // B2: @id cannot appear on an optional field — primary keys are NOT NULL.
    if is_id && field_def.field_type.is_optional {
        return Err(CoreError::Validation {
            message: format!(
                "Field `{}.{}` is marked `@id` but is optional; primary key columns cannot be NULL.",
                model_name, field_def.name,
            ),
            span: Some(field_def.span),
        });
    }

    // B3: @default(autoincrement()) only applies to integer scalars.
    if matches!(default, Some(ast::DefaultValue::AutoIncrement)) {
        let is_int_scalar = matches!(
            field_type,
            FieldKind::Scalar(ScalarType::Int | ScalarType::BigInt)
        );
        if !is_int_scalar {
            return Err(CoreError::InvalidDefault {
                model_name: model_name.to_string(),
                field_name: field_def.name.clone(),
                message: format!(
                    "`@default(autoincrement())` requires an integer field, got `{type_name}`",
                ),
                span: Some(field_def.span),
            });
        }
    }

    // B5: @relation `fields` and `references` lists must have the same length.
    for attr in &field_def.attributes {
        if let ast::FieldAttribute::Relation(rel) = attr
            && rel.fields.len() != rel.references.len()
        {
            return Err(CoreError::InvalidRelationFields {
                model_name: model_name.to_string(),
                field_name: field_def.name.clone(),
                message: format!(
                    "`@relation` `fields` (length {}) and `references` (length {}) must have the same length",
                    rel.fields.len(),
                    rel.references.len(),
                ),
                span: Some(field_def.span),
            });
        }
    }

    // Resolve @map
    let db_name = field_def
        .attributes
        .iter()
        .find_map(|a| match a {
            ast::FieldAttribute::Map(name) => Some(name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| to_snake_case(&field_def.name));

    // Resolve @relation
    let relation = field_def.attributes.iter().find_map(|a| match a {
        ast::FieldAttribute::Relation(rel) => {
            let relation_type = if field_def.field_type.is_list {
                RelationType::OneToMany
            } else if field_def.field_type.is_optional {
                RelationType::OneToOne
            } else {
                RelationType::ManyToOne
            };

            Some(ResolvedRelation {
                name: rel.name.clone(),
                related_model: type_name.clone(),
                relation_type,
                fields: rel.fields.clone(),
                references: rel.references.clone(),
                on_delete: rel.on_delete.unwrap_or(ast::ReferentialAction::Restrict),
                on_update: rel.on_update.unwrap_or(ast::ReferentialAction::Cascade),
            })
        }
        _ => None,
    });

    // Resolve @db.* type hint (e.g. @db.BigInt)
    let db_type = field_def.attributes.iter().find_map(|a| match a {
        ast::FieldAttribute::DbType(ty, args) => Some((ty.clone(), args.clone())),
        _ => None,
    });

    Ok(Field {
        name: field_def.name.clone(),
        db_name,
        field_type,
        is_optional: field_def.field_type.is_optional,
        is_list: field_def.field_type.is_list,
        is_id,
        is_unique,
        is_updated_at,
        default,
        relation,
        db_type,
    })
}

#[cfg(test)]
#[allow(clippy::pedantic)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use ferriorm_core::utils::to_snake_case;

    #[test]
    fn test_validate_basic_schema() {
        let source = r#"
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

generator client {
  output = "./src/generated"
}

enum Role {
  User
  Admin
}

model User {
  id    String @id @default(uuid())
  email String @unique
  name  String?
  role  Role   @default(User)

  @@map("users")
}
"#;

        let ast = parse(source).expect("parse");
        let schema = validate(&ast).expect("validate");

        assert_eq!(schema.datasource.provider, DatabaseProvider::PostgreSQL);
        assert_eq!(schema.enums.len(), 1);
        assert_eq!(schema.enums[0].name, "Role");
        assert_eq!(schema.enums[0].db_name, "role");

        let user = &schema.models[0];
        assert_eq!(user.name, "User");
        assert_eq!(user.db_name, "users");
        assert_eq!(user.primary_key.fields, vec!["id"]);

        let id_field = &user.fields[0];
        assert!(id_field.is_id);
        assert_eq!(id_field.field_type, FieldKind::Scalar(ScalarType::String));

        let name_field = &user.fields[2];
        assert!(name_field.is_optional);
        assert_eq!(name_field.db_name, "name");

        let role_field = &user.fields[3];
        assert_eq!(role_field.field_type, FieldKind::Enum("Role".into()));
    }

    #[test]
    fn test_validate_missing_primary_key() {
        let source = r#"
datasource db {
  provider = "postgresql"
  url      = "postgres://localhost/test"
}

model User {
  email String
  name  String
}
"#;

        let ast = parse(source).expect("parse");
        let err = validate(&ast).unwrap_err();
        assert!(matches!(err, CoreError::MissingPrimaryKey { .. }));
        assert!(err.span().is_some(), "missing-pk error should carry a span");
    }

    #[test]
    fn test_validate_unknown_type() {
        let source = r#"
datasource db {
  provider = "postgresql"
  url      = "postgres://localhost/test"
}

model User {
  id   String @id
  role Nonexistent
}
"#;

        let ast = parse(source).expect("parse");
        let err = validate(&ast).unwrap_err();
        assert!(matches!(err, CoreError::UnknownType { .. }));
        assert!(err.span().is_some());
    }

    #[test]
    fn test_validate_composite_primary_key() {
        let source = r#"
datasource db {
  provider = "sqlite"
  url      = "file:./dev.db"
}

model PostTag {
  postId String
  tagId  String

  @@id([postId, tagId])
}
"#;

        let ast = parse(source).expect("parse");
        let schema = validate(&ast).expect("validate");
        let model = &schema.models[0];
        assert_eq!(model.primary_key.fields, vec!["postId", "tagId"]);
        assert!(model.primary_key.is_composite());
    }

    #[test]
    fn test_snake_case() {
        assert_eq!(to_snake_case("User"), "user");
        assert_eq!(to_snake_case("PostTag"), "post_tag");
        assert_eq!(to_snake_case("createdAt"), "created_at");
        assert_eq!(to_snake_case("HTMLParser"), "h_t_m_l_parser");
    }

    #[test]
    fn test_validate_auto_table_name() {
        let source = r#"
datasource db {
  provider = "postgresql"
  url      = "postgres://localhost/test"
}

model BlogPost {
  id String @id
}
"#;

        let ast = parse(source).expect("parse");
        let schema = validate(&ast).expect("validate");
        // Auto-generated: snake_case + "s"
        assert_eq!(schema.models[0].db_name, "blog_posts");
    }
}
