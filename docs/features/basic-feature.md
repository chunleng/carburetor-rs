# Basic Feature

## Overview

Carburetor is a local-first framework that enables sync between a storage
backend and multiple frontend devices with LWW (Last Writes Win) Register.

It mainly revolves around the `#[carburetor]` proc macro which enables developers to add incremental sync capabilities to Diesel model structs. The macro generates data models that can be passed to the sync capabilities.

## Core Implementation Library/Framework/Tool

| Library/Framework/Tool | Purpose |
|---|---|
| Diesel | Database abstraction and ORM for PostgreSQL queries |
| PostgreSQL | Relational database backend for storing CRDT data with timestamps |
| Proc macro (syn, quote, proc-macro2) | Code generation to create the model type from annotated structs |

## Feature Components

### Macro Application

Developers apply the `#[carburetor(...)]` attribute to structs that represent Diesel models. The macro processes the struct definition and generates a corresponding model type.

### Generated Model

The macro generates a model—a zero-sized type or similar compile-time construct—that represents the struct metadata. This model encodes information about the struct (such as the table it maps to and timestamp column) in a form that can be passed to library functions.

### Incremental Sync Function

The library provides a `download_sync` function that accepts:
- A generated model (created by the macro)
- A `last_update_datetime` parameter indicating the sync point

The function queries the associated table for all records modified since `last_update_datetime` and returns:
- The changed records
- The current datetime up to which the sync is complete

## Challenges and Considerations

### Timestamp Column Identification

The macro must reliably identify or be configured with the timestamp column used for tracking updates. Clear configuration or naming conventions are needed to avoid ambiguity.

### Struct Field Compatibility

Not all struct field types may be compatible with the sync mechanism. Handling of custom types, unsupported columns, or serialization requirements needs definition.
