---
id: TASK-32
title: 'Epic: Multiuser (level one) — shared server for a small team'
status: To Do
assignee: []
created_date: '2026-08-06 16:27'
labels:
  - epic
  - multiuser
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Evolve nigel serve from single-user localhost into a shared server for a small team (2 to 10 users, one company database): real authentication and roles in front of the same JSON API, attribution and an audit trail on every write, and a supportable deployment story (LAN, Tailscale, or small VPS). SQLite stays — per-request connections, WAL, and busy timeouts already fit this scale. The SQLCipher database key remains server-side and separate from user identity: an admin unlocks the database at boot, users authenticate with their own credentials. Single-user localhost mode remains the zero-setup default. This epic is the accountability groundwork the invoicing add-on (tasks 1-3) needs — who issued, edited, and voided an invoice must be answerable. Explicitly out of scope: multi-tenant SaaS, Postgres, sync/CRDTs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Multiple users with individual credentials and roles can work against the same instance concurrently
- [ ] #2 Every write is attributed to a user and the audit log answers who changed what and when
- [ ] #3 A read-only role exists that an external accountant can safely use
- [ ] #4 Single-user localhost mode still works with zero auth configuration
- [ ] #5 A deployment guide covers LAN, Tailscale, and VPS setups with TLS
<!-- AC:END -->
