# Client migration: reset to clean state

When a client's local SQLite schema drifts in a way migration cannot repair, the
generated `run_migrations` does not fail permanently. It wipes the entire local
database, recreates the schema from scratch, and reports the wipe to the caller
as `Error::DatabaseWiped`. This document explains why the reset exists, why it
is as destructive as it is, and how the client recovers its data afterward.

## Why a reset at all

Client migrations can repair some drift in place - the table rebuild fallback
handles cases like NOT NULL relaxation. But other drift is unrecoverable
locally: a column type change, a primary key mismatch, or nullability tightening
cannot be fixed without knowing which data is trustworthy. When schema
validation detects such drift, it returns `Error::Migration`, and every
subsequent call to `run_migrations` would fail the same way. The app would be
stuck: unable to migrate, unable to sync.

The reset breaks that deadlock by refusing to trust the local database at all.
After unrecoverable drift, the local state cannot be relied upon, so instead of
attempting a partial repair that might fail again, the client starts over. This
is only viable because the data is not truly lost: the backend holds the synced
dataset, and the client can pull it back.

## Why the whole database, not just managed tables

The wipe removes every user table, view, and index in the database, discovered
by introspecting `sqlite_master` - not only the tables the sync system manages.
An unmanaged table the app created for its own purposes is destroyed just the
same.

This is deliberate. A partial wipe that preserves unmanaged entities would leave
the database in a mixed state: some schema recreated fresh, some predating the
failure. If the drift was caused by something outside the managed tables, a
partial wipe could recreate the same failure on the next run. Treating the
database as a whole is the safer contract: after a reset, the database is
exactly what a fresh install would have.

The cost is real data loss for anything the app kept locally without syncing it
to the backend. That trade-off is accepted because the reset only fires on
unrecoverable drift, where the alternative is a client that never works again.
It is a last resort, not a routine path.

## Why views and indexes are dropped too

In SQLite, tables, views, and indexes share a single namespace. A view or an
index named `messages` blocks `CREATE TABLE messages` just as effectively as an
existing table would. If the reset dropped only tables, a surviving view or
index occupying a managed table's name would turn the recovery into a hard
failure: the reset would succeed, but the schema recreation immediately after
would fail with "already exists".

So the reset drops all three entity types explicitly. Triggers need no separate
handling: they live in their own namespace and are dropped automatically when
their parent table or view goes away. SQLite's internal tables (`sqlite_%`, such
as `sqlite_sequence`) are excluded - they are not user entities and recreating
them is SQLite's job.

## What the caller sees

When the reset succeeds, `run_migrations` returns `Error::DatabaseWiped` with
the original drift error preserved as its source. The variant exists so callers
can distinguish "migration failed" from "migration failed and your local data
was destroyed" - an app that keeps unsynced local state should tell the user
what happened. The original error remains available for diagnostics.

A combined error carrying both the drift error and any reset error was
considered and deliberately left out. Knowing the reset itself failed adds
little beyond what the plain reset error already says, and the original drift
error is already visible in logs before the reset runs. Simpler error surface
won.

## Why a failed reset does not loop

If migration fails on the freshly recreated database, that failure propagates
directly to the caller instead of triggering another reset. A failure on a fresh
database means something is wrong with the code or the environment, not with
accumulated local state - looping resets would mask the bug while repeatedly
destroying data. The guard also has a practical side effect: because the reset
removes every entity that could block a fresh `CREATE TABLE`, there is no way to
construct a fresh-run failure in an end-to-end test, so the guard is verified by
inspection rather than by test.

## Recovery: the client behaves like a fresh install

The reset also wipes `carburetor_offsets`, the table tracking how far the client
has downloaded. That is what makes recovery automatic: with no offsets, the next
download starts from zero and the backend returns every row, restoring the full
synced dataset. The client ends up in the same state as a first-time install -
which is exactly the point. No special re-sync mode, no offset repair: the
ordinary sync flow does the work, because the ordinary sync flow is what a fresh
client uses.
