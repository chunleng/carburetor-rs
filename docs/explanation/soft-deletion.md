# Why records are soft-deleted

Carburetor never physically removes a row from the database. Deleting a record
sets its `is_deleted` flag to true, and the row stays where it is, still
carrying its sync metadata. This is a deliberate design choice, and it follows
directly from how sync works.

## The problem with physical deletion

Sync in carburetor is incremental and timestamp-based: each client remembers
how far it has downloaded (`last_synced_at` offsets), and the backend hands
over everything newer. A deletion has to travel through that same channel like
any other change.

A physically deleted row leaves no trace. The backend could not tell which
clients have already seen the row, and clients that had downloaded it would
keep their stale copy forever, with nothing ever telling them the record is
gone. The only way to make deletion propagate is to keep the row and mark it:
the deletion then travels as an ordinary row update, riding the exact same
`last_synced_at` machinery as inserts and updates.

## How a deletion travels

On the client, the generated `delete_<table>()` function does not issue a SQL
DELETE. It sets `is_deleted = true` and marks the row dirty as an UPDATE, so
the deletion is uploaded exactly like any other modification. The backend
stores the flag, and because the row keeps its `last_synced_at`, it continues
to participate in incremental sync: other clients pick it up on their next
download and apply the same flag locally.

Application code is not expected to look at tombstones. The generated
`active_<plural>()` query helpers filter out rows where `is_deleted` is true,
so day-to-day reads only ever see live records.

## Fresh clients do not need tombstones

There is one asymmetry worth knowing. A clean (initial) download filters out
deleted records, so a brand-new client never receives rows that were already
deleted before it first synced; transferring tombstones to it would be wasted
data. Incremental downloads have no such filter, because existing clients need
those rows to propagate the deletion. The trade-off is that the backend keeps
deleted rows indefinitely, but that cost is what makes deletion propagation
possible at all.

## No undelete

Deletions are one-way. There is no mechanism to resurrect a soft-deleted row:
doing so consistently across every client would add real complexity for a
rare need. If a record turns out to still be wanted, the simple path is to
create a new one.
