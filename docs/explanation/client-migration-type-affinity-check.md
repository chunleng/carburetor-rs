# Why client migration checks types by affinity class

When `run_migrations` reconciles an existing SQLite database against your
declared schema, it does not compare type strings literally. A column declared
`TEXT` in your schema is accepted in a database where the column was created as
`VARCHAR(255)`, but rejected where it was created as `INTEGER`. The comparison
happens at the level of SQLite's five type affinity classes: TEXT, NUMERIC,
INTEGER, REAL, and BLOB.

## Context: SQLite does not enforce declared types

SQLite's type system is unusual. A column's declared type string is not a
constraint; it only determines the column's *affinity*, which governs how values
are stored and compared. Two differently spelled types with the same affinity
behave identically at runtime. This is documented in SQLite's own rules for
determining column affinity: a type containing `INT` maps to INTEGER affinity,
`CHAR`, `CLOB`, or `TEXT` to TEXT affinity, `BLOB` (or an empty type) to BLOB,
`REAL`, `FLOA`, or `DOUB` to REAL, and everything else to NUMERIC.

Because the type string itself carries no behavioral meaning beyond its
affinity, comparing declared and existing type strings exactly would be checking
the wrong thing.

## The decision: compare affinity classes

The migration validates an existing column by mapping both the declared type and
the database's type through these affinity rules and comparing the resulting
classes. This matches the level at which SQLite itself defines type
compatibility.

The practical consequences cut both ways:

- `VARCHAR(255)` vs `TEXT` is accepted. Both have TEXT affinity, so the columns
  behave identically. A database created by another tool or an earlier version
  of your app is not rejected over spelling.
- `TEXT` vs `INTEGER` is rejected. The affinities differ, so the columns
  genuinely store and compare values differently. Accepting this would let a
  schema declaration drift from what the database actually does.

An exact string comparison would get both cases wrong: it would reject
legitimate databases in the first case and, if loosened with aliases, could
accept incompatible ones in the second.

The error message names both the declared and the database type along with their
affinities, so a mismatch is diagnosable without knowing the affinity rules.

## Tradeoff: coarse but honest

Affinity comparison is deliberately coarse. It cannot catch every meaningful
difference, for example `BIGINT` and `INTEGER` share INTEGER affinity even
though they are spelled differently in the generated DDL. This is consistent
with SQLite's own model: since SQLite does not enforce the distinction, the
migration does not either. The tradeoff is that some differences a stricter
checker would flag are tolerated, in exchange for never rejecting a database
that actually behaves as declared.

## Related

- [How to set up a client with auto-migration](../how-to/setup-client-auto-migration.md)
- [Why some client migrations rebuild the whole table](client-migration-table-rebuild.md)
