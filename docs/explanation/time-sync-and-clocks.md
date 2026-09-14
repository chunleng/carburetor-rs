# Why sync time comes from two different clocks

Carburetor sync relies on timestamps in two places: the backend stamps every
uploaded record with `last_synced_at`, and the client stamps every locally
changed record with `dirty_at`. Both are UTC, but they are read from different
clocks. The backend's timestamps come from PostgreSQL itself, while the
client's come from the local device clock. This split is deliberate, and
understanding it explains both how incremental download stays correct and what
can go wrong on the client side.

## The backend: PostgreSQL time as the source of truth

When the backend processes a download request, it does not use the backend
process's wall clock. The generated download code asks the database for the
current time (`SELECT CURRENT_TIMESTAMP` through
`carburetor::helpers::get_db_utc_now`) and uses that value both to filter
records and as the `cutoff_at` returned to the client. The same is true on the
upload path: `last_synced_at` is stamped from the database write, and the
column's SQL default is `now()`.

The reason is incremental download. A client downloads records with
`last_synced_at` greater than its stored offset and at most the current
process time. If that "current time" came from the backend process's wall
clock, and more than one backend server is running, the clocks would not be
synchronized. A small timing difference is enough to miss records: a record
stamped by one server could fall outside the window another server computes.
PostgreSQL time removes this failure mode, because every backend instance
reads the same clock no matter which machine it runs on. Correct timing
matters more here than anywhere else in the sync flow, which is why the
database, not the process, is the source of truth.

## The client: the device clock, with a known trade-off

The client has no shared authority to consult, so `dirty_at` is set from the
local device clock (`chrono::Utc::now()`). The upload path uses the same clock
to select dirty records: it takes a cutoff of "now" on the device, uploads
everything with `dirty_at` at or before that cutoff, and clears the dirty
flags for records the backend confirmed.

This means a device clock change can cause data not to be uploaded. If the
clock jumps backward after a record is marked dirty, the record can fall
outside the cutoff window and be skipped. This is a known and accepted
trade-off: the chance of it happening is low, and the alternative, turning
`dirty_at` into a vector clock, would be much harder to implement and reason
about. The simpler design was chosen with the residual risk acknowledged
rather than engineered away.

## What this means in practice

The two timestamps never need to agree with each other, so the split does not
affect correctness of conflict resolution: `last_synced_at` orders records
against other records stamped by the same PostgreSQL clock, and `dirty_at`
only drives which local changes still need uploading. The practical
consequences are the ones above: backend deployments can scale horizontally
without clock concerns, while a client with a misbehaving clock may fail to
upload some changes until its clock is consistent again.
