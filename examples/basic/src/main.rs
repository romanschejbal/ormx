mod generated;

use generated::OrmxClient;
use ormx_runtime::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/ormx_example".into());

    println!("Connecting to {database_url}...");
    let client = OrmxClient::connect(&database_url).await?;

    // ─── CREATE ────────────────────────────────────────────
    let user = client
        .user()
        .create(generated::user::data::UserCreateInput {
            email: "alice@example.com".into(),
            name: Some("Alice".into()),
            role: Some(generated::Role::Admin),
            id: None,         // auto-generated uuid
            created_at: None, // auto-generated now()
        })
        .exec()
        .await?;
    println!("Created user: {} (id={})", user.email, user.id);

    // ─── FIND UNIQUE ───────────────────────────────────────
    let found = client
        .user()
        .find_unique(generated::user::filter::UserWhereUniqueInput::Email(
            "alice@example.com".into(),
        ))
        .exec()
        .await?;
    println!("Found: {:?}", found.map(|u| u.email));

    // ─── FIND MANY with filters ────────────────────────────
    let users = client
        .user()
        .find_many(generated::user::filter::UserWhereInput {
            email: Some(StringFilter {
                contains: Some("@example.com".into()),
                ..Default::default()
            }),
            ..Default::default()
        })
        .order_by(generated::user::order::UserOrderByInput::CreatedAt(
            SortOrder::Desc,
        ))
        .take(10)
        .exec()
        .await?;
    println!("Found {} users", users.len());

    // ─── UPDATE ────────────────────────────────────────────
    let updated = client
        .user()
        .update(
            generated::user::filter::UserWhereUniqueInput::Id(user.id.clone()),
            generated::user::data::UserUpdateInput {
                name: Some(SetValue::Set(Some("Alice Smith".into()))),
                ..Default::default()
            },
        )
        .exec()
        .await?;
    println!("Updated: {} -> {:?}", updated.email, updated.name);

    // ─── COUNT ─────────────────────────────────────────────
    let count = client
        .user()
        .count(generated::user::filter::UserWhereInput::default())
        .exec()
        .await?;
    println!("Total users: {count}");

    // ─── DELETE ────────────────────────────────────────────
    let deleted = client
        .user()
        .delete(generated::user::filter::UserWhereUniqueInput::Id(
            user.id.clone(),
        ))
        .exec()
        .await?;
    println!("Deleted: {}", deleted.email);

    client.disconnect().await;
    Ok(())
}
