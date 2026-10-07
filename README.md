# RustORM CLI

RustORM CLI is the schema and code-generation toolchain for RustORM.

The CLI is responsible for reading `rust.schema`, parsing and validating the schema, generating Rust entities, and eventually generating database migrations.

The CLI is separate from the RustORM runtime.

---

## Architecture

```text
rust.schema
    │
    ▼
Lexer
    │
    ▼
Parser
    │
    ▼
Schema AST
    │
    ▼
Validator
    │
    ├──────────────► Entity Generator
    │                       │
    │                       ▼
    │                generated/entities.rs
    │
    └──────────────► Migration Generator
                            │
                            ▼
                       migrations/
```

### Runtime Boundary

```text
RustORM CLI
    │
    ├── Schema parsing
    ├── Schema validation
    ├── Entity generation
    └── Migration generation
              │
              ▼
       RustORM Runtime
```

The CLI does **not** reimplement the ORM runtime.

Generated Rust code depends on the existing RustORM runtime.

---

# 1. Schema Language

## Models

```text
model User {
    id Int @id @default(autoincrement())
    name String
}
```

Status: **Complete**

## Scalar Types

```text
Int
String
Boolean
Float
DateTime
Decimal
Json
```

Status: **Complete in AST/parser**

## Nullable Fields

```text
email String?
```

Status: **Complete**

## Array Fields

```text
tags String[]
```

Status: **Complete**

## Model References

```text
author User
```

Status: **Complete**

## Field Attributes

```text
@id
@unique
@default(...)
@map(...)
@updatedAt
@relation(...)
```

Status: **Complete**

## Relation Options

```text
name
fields
references
onDelete
```

Status: **Complete**

Supported referential actions:

```text
Cascade
Restrict
SetNull
NoAction
```

Status: **Complete**

## Model Attributes

```text
@@map(...)
@@index(...)
@@unique(...)
```

Status: **Complete**

---

# 2. Lexer

The lexer converts schema source code into tokens.

Status:

```text
Identifiers                  ✅
Strings                     ✅
Numbers                     ✅
Symbols                     ✅
Whitespace handling         ✅
Comments                    ✅
Invalid character detection ✅
Unterminated string errors  ✅
```

---

# 3. Parser

The parser converts tokens into the Schema AST.

Status:

```text
Models                      ✅
Fields                      ✅
Scalar types                ✅
Model references            ✅
Nullable fields             ✅
Array fields                ✅
Field attributes            ✅
Model attributes            ✅
Relations                   ✅
Relation arguments          ✅
Referential actions         ✅
Default values              ✅
```

---

# 4. Schema AST

Current AST contains:

```text
Schema
Model
Field
FieldType
FieldAttribute
DefaultValue
RelationAttribute
ReferentialAction
ModelAttribute
```

Status: **Complete for current V0.1 schema**

---

# 5. Validator

The validator checks semantic correctness after parsing.

Current validations:

```text
Duplicate models                    ✅
Duplicate fields                    ✅
Unknown model references            ✅
Missing primary key                 ✅
Multiple primary keys               ✅
Unknown relation fields             ✅
Unknown relation references         ✅
Relation fields/references mismatch ✅
```

Status: **V0.1 complete**

More advanced validation will be added as the schema language grows.

---

# 6. Entity Generator

The entity generator converts the validated Schema AST into Rust source code compatible with the RustORM runtime.

Current output:

```text
Model struct              ✅
Entity struct             ✅
Entity implementation     ✅
Column definitions        ✅
Field constants           ✅
Output file generation    ✅
```

Current generated file:

```text
generated/entities.rs
```

Example:

```rust
#[derive(Debug, sqlx::FromRow)]
pub struct UserModel {
    pub id: i32,
    pub name: String,
}

pub struct User;

impl Entity for User {
    type Model = UserModel;
    const TABLE: &'static str = "users";

    const COLUMNS: &'static [Column] = &[
        Column::new("id"),
        Column::new("name"),
    ];
}
```

## Entity Generator — Remaining

```text
Scalar type generation       ⏳
Nullable type generation     ⏳
Array type generation        ⏳
@map support                 ⏳
@@map support                ⏳
Relation generation          ⏳
Create structs               ⏳
Update structs               ⏳
```

---

# 7. Migration Generator

The migration generator will convert schema information into database migrations.

## Planned

```text
Read schema                         ⏳
CREATE TABLE                        ⏳
Columns                             ⏳
Primary keys                        ⏳
Unique constraints                  ⏳
Indexes                             ⏳
Foreign keys                        ⏳
ON DELETE actions                   ⏳
ALTER TABLE                         ⏳
Migration files                     ⏳
Migration history                   ⏳
```

---

# 8. CLI Commands

Planned commands:

```text
rustorm generate
rustorm validate

rustorm migrate
rustorm migrate dev
rustorm migrate status
rustorm migrate reset
```

Status: **Not implemented**

The current executable temporarily accepts the schema path as an argument and defaults to:

```text
rust.schema
```

---

# 9. Project Integration

Target project structure:

```text
myproject/
├── rust.schema
├── migrations/
└── myrustproject/
    ├── Cargo.toml
    └── src/
        └── main.rs
```

Target workflow:

```text
rustorm generate
```

generates Rust entities.

```text
rustorm migrate dev
```

creates/applies development migrations.

Generated application code should use the existing RustORM runtime:

```rust
use rustorm::prelude::*;

let users = User::find()
    .all(&db)
    .await?;
```

---

# 10. Testing

Current test status:

```text
Lexer tests                 ✅
Parser tests                ✅
AST tests                   ✅
Validator tests             ✅
Generator tests             ✅

Current total: 27 tests
```

Current verification:

```text
cargo test
```

Expected:

```text
27 passed
```

Clippy verification:

```text
cargo clippy --all-targets --all-features -- -D warnings
```

Current status:

```text
0 warnings
0 errors
```

---

# 11. Development Order

The implementation order is intentionally:

```text
1. Schema Language
       ↓
2. Lexer
       ↓
3. Parser
       ↓
4. AST
       ↓
5. Validator
       ↓
6. Entity Generator
       ↓
7. Migration Generator
       ↓
8. CLI Commands
       ↓
9. Project Integration
       ↓
10. End-to-End Testing
```

Do not implement later stages prematurely.

---

# 12. Current Milestone

Current milestone:

**V0.1 — Schema → Validated AST → Basic Entity Generation**

Completed:

```text
Schema parsing             ✅
AST                         ✅
Validation                  ✅
Basic entity generation     ✅
Generated file output       ✅
27 tests                    ✅
Clippy clean                ✅
```

Next milestone:

```text
Complete Entity Generator
```

After that:

```text
Migration Generator
```

Then:

```text
CLI commands + project integration
```

---

# 13. Design Principle

The RustORM CLI should remain a **toolchain**, not a second ORM runtime.

```text
CLI
 ├── Understand schema
 ├── Validate schema
 ├── Generate code
 └── Generate migrations

Runtime
 ├── Connect to database
 ├── Build queries
 ├── Execute queries
 ├── Manage entities
 └── Manage runtime ORM behavior
```

This separation should be maintained as the project grows.