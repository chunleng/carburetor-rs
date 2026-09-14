# `#[default]` column attribute

Sets a default value for a data column. Two forms: `rust` embeds a Rust
expression in generated insert code; `sql` emits a DDL `DEFAULT` clause (with
the migration feature) or acts as a marker (without it).

## Signature

```rust
#[default(rust = "<Rust expression>")]
#[default(sql = <variant>)]   // with migration feature
#[default(sql)]               // without migration feature
```

## Example Usage

```rust
carburetor_sync_config! {
    tables {
        user {
            #[default(rust = "\"active\"")]
            status -> Text,
            #[default(sql = Now)]
            created_at -> Timestamptz,
            #[default(sql = Number(0))]
            login_count -> Integer,
        }
    }
}
```

## Applicability

The following apply to every form of `#[default]`:

- More than one `#[default]` on a column fails compilation
- Applying `#[default]` to a special column (`#[id]`, `#[last_synced_at]`,
  `#[is_deleted]`, `#[dirty_flag]`, `#[client_column_sync_metadata]`) fails
  compilation; their defaults are intrinsic

### `rust = "<rust_expr>"`

- `rust_expr`: any valid rust expression that can be used to assign value to the
  type. The expression must produce the column's Rust type:

  | SQL type | Rust type |
  | --- | --- |
  | `Text` | `String` |
  | `SmallInt` | `i16` |
  | `Integer` | `i32` |
  | `BigInt` | `i64` |
  | `Float` | `f32` |
  | `Double` | `f64` |
  | `Bool` | `bool` |
  | `Timestamp` | `carburetor::chrono::NaiveDateTime` |
  | `Timestamptz` | `carburetor::chrono::DateTimeUtc` |
  | `Date` | `carburetor::chrono::NaiveDate` |
  | `Time` | `carburetor::chrono::NaiveTime` |
  | `Jsonb` | `carburetor::serde_json::Value` |
  | `Nullable<T>` | `Option<T>` |

### `sql = <variant>`

- This can be used when cargo feature `migration` is included.
- The variant must match the column type (migration feature):
  - `Now`: `Timestamptz`, `Timestamp`, `Date`, `Time`, and their nullable
    variants
  - `EmptyJson`: `Jsonb`, `Nullable<Jsonb>`
  - `Text`: `Text`, `Nullable<Text>`
  - `Number`: `SmallInt`, `Integer`, `BigInt`, `Float`, `Double`, and their
    nullable variants
  - `Null`: `Nullable<...>` only

### `sql` (bare form)

- This can be used when cargo feature `migration` is omitted.
- The user manages schema defaults themselves (i.e. table column in `CREATE
  TABLE` needs a `DEFAULT` modifier)

## Effect on generated code

Defaults are applied in two general locations:

- New records on the client: the default fills the column on local insert.
- Uploading without a value: when a new column with a `#[default]` is added to
  the backend, a client on an older version does not send it. The backend fills
  the missing value from the default. This keeps uploads from old clients
  working after the schema gains new columns.

Because of that, the following fields are padded with `Option` to allow setting
default value:

- `Insert{Table}` on client: allow new records created on client-side to be created with `DEFAULT`
- `UploadInsert{Table}` on backend: allow newly synced records uploaded to be
  created with `DEFAULT`
- `Insertable{Table}` on client/backend with the `sql` default: this allows
  passing record that should be created on the DB to accept `DEFAULT` values

### How generated code accepts a default value

Generated structs pad columns with a `#[default]` to `Option<T>` so the value
can be omitted (`None` = use the default).

Contrast on `status -> Text`, with and without a default:

Without `#[default]`, the generated local insert model requires the field:

```rust
pub struct InsertUser {
    pub status: String, // must be supplied
}
```

With `#[default(rust = "\"active\"")]`, the field is padded to `Option<T>`:

```rust
pub struct InsertUser {
    pub status: Option<String>, // None = use the default
}
```

The generated conversion applies the fallback:

```rust
status: value.status.unwrap_or_else(|| "active")
```

### `Nullable<T>` columns and `diesel::Insertable`

For a `Nullable<T>` column with a `#[default]`, the padded field is
`Option<Option<T>>`:

- `None`: diesel emits the `DEFAULT` keyword for the column in the `INSERT`
  statement (diesel's default behavior), so the database default applies
- `Some(None)`: the column is inserted as `NULL`
- `Some(Some(v))`: `v` is inserted

## Usage notes

- The special columns have intrinsic defaults, and these are Rust defaults, not
  SQL defaults: `#[last_synced_at]` = `diesel::dsl::now`, `#[dirty_flag]` =
  `None`, `#[client_column_sync_metadata]` = `{}` (as JSON). They produce no
  DDL `DEFAULT` clause.
- `#[default]` cannot change these intrinsic defaults; applying it to a special
  column fails compilation.
