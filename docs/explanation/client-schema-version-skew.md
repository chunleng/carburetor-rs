# Client schema version skew: solving the problem

The backend and its clients do not upgrade in lockstep. A backend deployed with
a new column will happily send that column to clients still running an older
schema - clients that have no place to put the data. Rather than dropping it,
the client stages the unknown values and applies them later, once a migration
has caught the local schema up. This document explains why the staging exists and
why `apply_backfill` behaves the way it does.

## The problem: rows synced before the upgrade

The skew that matters is not the first download after the backend gains a
column - it is everything already synced before that. A row downloaded by the
older client carries no trace of the new column, and incremental sync never
re-sends it: downloads only deliver rows with `last_synced_at` past the client's
offset. So when the client later upgrades and migrates, the new column exists
locally but holds nothing for every row synced before the upgrade, while the
backend holds real values for those same rows. The column would be silently
wrong, and no amount of waiting fixes it, because the data was dropped at
download time and the sync cycle has no reason to deliver it again.

Staging avoids that: the unknown values are captured at download time and
applied after the upgrade.

## How staging works

Data arrives with an extra field the client schema does not know. Instead of
dropping it, the client stores the extra field in the row's metadata, keyed by
column name. Nothing is dropped and nothing is invented: the values are exactly
what the backend sent.

Staging is self-maintaining. Each newer update from the backend overwrites the
staged value, so the staged value is always the latest server value. A staged
column simply waits for its column to appear.

Waiting ends when a migration creates the column. Applying is the job of
`apply_backfill`, which every sync group generates: it moves each staged value
from the staged metadata into the actual data column it names; entries whose
columns still do not exist stay staged for a later attempt. If a staged value
cannot be converted to the column's type, the table is reset: the sync offset
is dropped and the staged data is cleared. No data is lost: the table is
re-fetched from scratch by the ordinary sync cycle, which converges on the
backend's state.

## Related documents

- [`sync_groups` reference](../reference/sync-config/sync-group.md) -
  `apply_backfill` reference
- [Tables reference](../reference/sync-config/table.md) -
  `#[client_column_sync_metadata]` staging
