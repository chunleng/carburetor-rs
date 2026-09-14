# Why Sync Groups Exist

Carburetor lets you declare many sync groups within a single sync config, and
the natural question is why the split exists at all: if every table is defined
once, why not sync everything through one group? The sync group allows the
developer to create consumers of different requirements from a single data
source, as we will explore further in this document.

## What a Sync Group Is

Tables are defined once in the `tables` block and referenced by name inside sync
group entries, so every group shares the same table definitions. What a group
owns is its own sync behavior: each group generates its own download and upload
functions and its own request and response models.

## Decision: Different Requirements Lead to Separate Groups

The primary reason multiple sync groups exist is that different applications or
consumers have totally different requirements. Sync groups allow controlling
the data that a consumer will receive, and this helps the client in many ways:
saving data, by removing irrelevant data from being downloaded, and setting
different permissions, by only sharing relevant data with the correct user.

Rather than one group with options to toggle these behaviors, each combination
of requirements becomes its own group. A game application is the canonical
example: a mobile group syncing limited, published content; a web group syncing
full game data; and an admin group with complete access including audit logs.
The same tables serve all three, but the requirements are different enough that
forcing them into one configurable group would make every consumer pay for
options it does not use.
