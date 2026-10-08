# RustORM CLI

RustORM CLI is the schema and code-generation toolchain for RustORM.

The CLI is responsible for reading `rust.schema`, parsing and validating the schema, generating Rust entities, and eventually generating database migrations.

The CLI is separate from the RustORM runtime.

---

# Architecture

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

## Runtime Boundary

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

## Many-to-Many Relation Options

```text
through
pivotFrom
pivotTo
```

Status: **Complete**

Example:

```text
model User {
    id Int @id

    roles Role[] @relation(
        through: "user_roles",
        pivotFrom: "user_id",
        pivotTo: "role_id"
    )
}
```

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

Current status:

```text
Identifiers                   ✅
Strings                      ✅
Numbers                      ✅
Symbols                      ✅
Whitespace handling           ✅
Comments                      ✅
Invalid character detection   ✅
Unterminated string errors    ✅
```

---

# 3. Parser

The parser converts tokens into the Schema AST.

Current status:

```text
Models                       ✅
Fields                       ✅
Scalar types                 ✅
Model references             ✅
Nullable fields              ✅
Array fields                 ✅
Field attributes             ✅
Model attributes             ✅
Relations                    ✅
Relation arguments           ✅
Referential actions          ✅
Default values               ✅
Many-to-many arguments       ✅
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
Duplicate fields                   ✅
Unknown model references           ✅
Missing primary key                ✅
Multiple primary keys              ✅
Unknown relation fields             ✅
Unknown relation references         ✅
Relation fields/references mismatch ✅
Incomplete many-to-many relations   ✅
Mixed relation metadata             ✅
```

Status: **V0.1 complete**

More advanced validation will be added as the schema language grows.

Planned future validation includes:

```text
Relation field type compatibility   ⏳
Relation cardinality validation     ⏳
Relation key type validation        ⏳
Invalid @default validation         ⏳
Invalid @unique usage               ⏳
Invalid @@index fields              ⏳
Invalid @@unique fields             ⏳
Invalid referential actions         ⏳
```

---

# 6. Entity Generator

The entity generator converts the validated Schema AST into Rust source code compatible with the RustORM runtime.

## Current Output

```text
Model struct                  ✅
Entity struct                 ✅
Entity implementation         ✅
Column definitions            ✅
Field constants               ✅
Output file generation        ✅
Scalar Rust types             ✅
Nullable Rust types           ✅
Array Rust types              ✅
@map support                  ✅
@@map support                 ✅
```

Generated file:

```text
generated/entities.rs
```

## Example

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

## Relations

```text
One-to-Many                  ✅
Many-to-One                  ✅
One-to-One                   ✅
RelationKey                  ✅
@map relation keys            ✅
Many-to-Many                 ✅
```

Many-to-many generation includes:

```text
Vec<Arc<TargetModel>>
RelationLoader implementation
Relation::many_to_many(...)
Pivot table
Pivot source column
Pivot target column
```

## Create / Update Generation

```text
Create structs                ✅
InsertData generation         ⏸️
Update structs                ⏸️
UpdateData generation         ⏸️
```

Create structs currently:

- exclude primary key fields
- exclude relation fields
- preserve scalar Rust types
- preserve nullable types
- preserve array types

Example:

```rust
pub struct UserCreate {
    pub name: String,
    pub email: Option<String>,
}
```

### Why InsertData Is Paused

The generated `InsertData` implementation depends on the runtime `BindValue` system.

Current RustORM runtime:

```rust
pub enum BindValue {
    String(String),
    I64(i64),
}
```

The schema language supports additional types:

```text
Boolean
Float
DateTime
Decimal
Json
nullable values
```

Therefore `InsertData` generation should continue after the runtime binding layer has been expanded.

Current dependency direction:

```text
RustORM Runtime
    │
    ├── Expand BindValue
    ├── Update executor binding
    └── Add runtime tests
            │
            ▼
RustORM CLI
    │
    ├── InsertData generation
    ├── Update structs
    └── UpdateData generation
```

The CLI must not work around an incomplete runtime binding layer.

---

# 7. Migration Generator

The migration generator will convert schema information into database migrations.

## Planned

```text
Read schema                  ⏳
Detect new models            ⏳
CREATE TABLE                 ⏳
Columns                      ⏳
Primary keys                 ⏳
Unique constraints           ⏳
Indexes                      ⏳
Foreign keys                 ⏳
ON DELETE actions            ⏳
ALTER TABLE                  ⏳
Add columns                  ⏳
Drop columns                 ⏳
Rename columns               ⏳
Add/remove indexes           ⏳
Join tables                  ⏳
Migration files              ⏳
Migration history            ⏳
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

Current execution pipeline:

```text
Read schema
    ↓
Lexer
    ↓
Parser
    ↓
Validator
    ↓
Entity Generator
    ↓
generated/entities.rs
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

```bash
rustorm generate
```

generates Rust entities.

```bash
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

Planned integration:

```text
Detect rust.schema              ⏳
Detect Cargo project            ⏳
Generated directory             ⏳
Generated module integration    ⏳
Runtime crate integration       ⏳
Project configuration           ⏳
```

---

# 10. Testing

Current test status:

```text
Lexer tests                    ✅
Parser tests                   ✅
AST tests                      ✅
Validator tests                ✅
Generator tests                ✅
```

Current total:

```text
42 tests
```

Current verification:

```bash
cargo test
```

Result:

```text
42 passed
0 failed
```

Clippy verification:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Result:

```text
0 warnings
0 errors
```

The current CLI checkpoint is therefore:

```text
42 tests passing
Clippy clean
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

Within the Entity Generator, runtime-dependent CRUD generation is developed only after the runtime APIs required by generated code are ready.

Do not implement later stages prematurely.

---

# 12. Current Milestone

Current milestone:

**V0.1 — Schema → Validated AST → Entity Generation**

Completed:

```text
Schema parsing                 ✅
AST                            ✅
Validation                     ✅
Model generation               ✅
Entity generation              ✅
Field generation               ✅
Relation generation            ✅
Many-to-many generation        ✅
RelationKey generation         ✅
Create struct generation       ✅
Generated file output           ✅
42 tests                       ✅
Clippy clean                   ✅
```

Current paused work:

```text
InsertData generation          ⏸️
Update struct generation       ⏸️
UpdateData generation          ⏸️
```

Reason:

The RustORM runtime's `BindValue` and executor binding layer currently support only a subset of the schema language's scalar/nullable values.

## Next Runtime Milestone

Before continuing CRUD code generation in the CLI:

```text
1. Expand BindValue
        ↓
2. Update executor binding
        ↓
3. Add runtime tests
        ↓
4. Return to CLI InsertData generation
        ↓
5. Generate Update structs
        ↓
6. Generate UpdateData
```

After Entity Generator completion:

```text
Migration Generator
        ↓
CLI Commands
        ↓
Project Integration
        ↓
End-to-End Testing
```

---

# 13. Design Principle

The RustORM CLI should remain a **toolchain**, not a second ORM runtime.

```text
CLI
├── Understand schema
├── Parse schema
├── Validate schema
├── Generate Rust code
└── Generate migrations

Runtime
├── Connect to database
├── Build queries
├── Execute queries
├── Manage entities
└── Manage runtime ORM behavior
```

The CLI generates code for the existing RustORM runtime.

The CLI should not duplicate runtime functionality.

This separation should be maintained as the project grows.