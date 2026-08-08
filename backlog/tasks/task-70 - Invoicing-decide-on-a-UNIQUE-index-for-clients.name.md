---
id: TASK-70
title: 'Invoicing: decide on a UNIQUE index for clients.name'
status: To Do
assignee: []
created_date: '2026-08-08 08:21'
labels:
  - invoicing
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
add_client/update_client refuse duplicate names in the data layer (advisory — two racing web clients can still both insert, since clients.name carries no UNIQUE constraint). Decide whether to add the index by migration: existing databases (and InvoiceShelf imports) may already hold duplicates, so the migration needs a dedup or rename strategy before the constraint can land. Surfaced during TASK-68.6 stage 3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Either a UNIQUE index exists with a migration that handles pre-existing duplicates, or the advisory-only behavior is documented as deliberate
<!-- AC:END -->
