# Relations

Relations connect models to each other and let you query across tables. ferriorm supports one-to-one, one-to-many, and implicit many-to-many relationships (via join models).

## Key Concepts

Before looking at examples, there are a few things to understand about how relations work in ferriorm:

1. **The side with `@relation` stores the foreign key.** The `@relation` attribute specifies which field(s) on the current model hold the foreign key and which field(s) on the related model they reference.

2. **Relation fields are virtual.** A field whose type is another model (e.g., `author User`) does _not_ create a database column. It tells the code generator to produce a relation accessor. The actual foreign key is stored in a separate scalar field (e.g., `authorId String`).

3. **Both sides must be defined.** The model that owns the foreign key has the `@relation` attribute; the other model has a back-relation field (plain model type or list type) with no `@relation`.

---

## One-to-Many

The most common relation type. One record in the parent model is related to many records in the child model.

```prisma
model User {
  id    String @id @default(uuid())
  posts Post[]
}

model Post {
  id       String @id @default(uuid())
  author   User   @relation(fields: [authorId], references: [id])
  authorId String
}
```

**What this means:**

| Part | Purpose |
|---|---|
| `posts Post[]` on `User` | Back-relation (virtual). A user has many posts. Not a database column. |
| `author User @relation(...)` on `Post` | Relation field (virtual). Represents the related user. Not a database column. |
| `authorId String` on `Post` | Foreign key (real column). Stores the `User.id` value in the database. |

The `fields: [authorId]` argument tells ferriorm that `Post.authorId` is the foreign key, and `references: [id]` says it points to `User.id`.

### Generated SQL (PostgreSQL)

```sql
CREATE TABLE "users" (
  "id" TEXT NOT NULL PRIMARY KEY
);

CREATE TABLE "posts" (
  "id" TEXT NOT NULL PRIMARY KEY,
  "authorId" TEXT NOT NULL REFERENCES "users"("id")
);
```

---

## One-to-One

A one-to-one relation is like a one-to-many except the foreign key side has a `@unique` constraint, ensuring only one related record can exist.

```prisma
model User {
  id      String   @id @default(uuid())
  profile Profile?
}

model Profile {
  id     String @id @default(uuid())
  bio    String?
  user   User   @relation(fields: [userId], references: [id])
  userId String @unique
}
```

**What makes this one-to-one:**

- `Profile.userId` has `@unique`, so each user can have at most one profile.
- `User.profile` is typed as `Profile?` (optional), because a user may or may not have a profile.

### Generated SQL (PostgreSQL)

```sql
CREATE TABLE "users" (
  "id" TEXT NOT NULL PRIMARY KEY
);

CREATE TABLE "profiles" (
  "id" TEXT NOT NULL PRIMARY KEY,
  "bio" TEXT,
  "userId" TEXT NOT NULL UNIQUE REFERENCES "users"("id")
);
```

---

## Many-to-Many (via Join Model)

For many-to-many relationships, create an explicit join model with two foreign keys:

```prisma
model Post {
  id   String    @id @default(uuid())
  tags PostTag[]
}

model Tag {
  id    String    @id @default(uuid())
  name  String    @unique
  posts PostTag[]
}

model PostTag {
  post   Post   @relation(fields: [postId], references: [id])
  postId String
  tag    Tag    @relation(fields: [tagId], references: [id])
  tagId  String

  @@id([postId, tagId])
}
```

The `PostTag` join model:
- Has two foreign keys (`postId` and `tagId`) linking to `Post` and `Tag`
- Uses `@@id([postId, tagId])` to create a composite primary key
- Is a real table in the database

---

## Referential Actions

Referential actions control what happens when a referenced record is deleted or updated. Specify them with the `onDelete` and `onUpdate` arguments in `@relation`.

```prisma
model Post {
  id       String @id @default(uuid())
  author   User   @relation(fields: [authorId], references: [id], onDelete: Cascade)
  authorId String
}
```

### Available Actions

| Action | On Delete | On Update |
|---|---|---|
| `Cascade` | Delete all posts when the user is deleted | Update foreign keys when the user's id changes |
| `Restrict` | Prevent deleting a user who has posts | Prevent updating a user's id if posts reference it |
| `NoAction` | Similar to Restrict (exact behavior is database-dependent) | Similar to Restrict |
| `SetNull` | Set `authorId` to `NULL` when the user is deleted (field must be optional) | Set `authorId` to `NULL` when the user's id changes |
| `SetDefault` | Set `authorId` to its default value when the user is deleted | Set `authorId` to its default value when the user's id changes |

### Example: Cascade Delete

When a user is deleted, all their posts are automatically deleted:

```prisma
model User {
  id    String @id @default(uuid())
  posts Post[]
}

model Post {
  id       String @id @default(uuid())
  author   User   @relation(fields: [authorId], references: [id], onDelete: Cascade)
  authorId String
}
```

### Example: Set Null

When a user is deleted, posts are kept but `authorId` is set to `NULL`:

```prisma
model User {
  id    String @id @default(uuid())
  posts Post[]
}

model Post {
  id       String  @id @default(uuid())
  author   User?   @relation(fields: [authorId], references: [id], onDelete: SetNull)
  authorId String?
}
```

Note that both `author` and `authorId` must be optional (`?`) for `SetNull` to work.

---

## Self-Relations

A model can relate to itself. For example, a tree structure where each category can have a parent:

```prisma
model Category {
  id       String     @id @default(uuid())
  name     String
  parent   Category?  @relation("CategoryTree", fields: [parentId], references: [id])
  parentId String?
  children Category[] @relation("CategoryTree")
}
```

The string `"CategoryTree"` is a **relation name** that disambiguates the two relation fields on the same model. Both sides must use the same relation name.

---

## Rules and Constraints

- Every `@relation` must specify `fields` and `references`.
- The `fields` array lists foreign key fields on the current model.
- The `references` array lists the corresponding key fields on the related model.
- The number of entries in `fields` and `references` must match.
- Relation fields (model-typed fields) are **not** stored in the database. Only the scalar foreign key fields become columns.
- The back-relation side (the side without `@relation`) uses either a list type (`Post[]`) for one-to-many or an optional type (`Profile?`) for one-to-one.
