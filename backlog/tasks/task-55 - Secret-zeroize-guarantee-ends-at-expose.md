---
id: TASK-55
title: Secret's zeroize guarantee ends at the first expose()
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - security
  - tech-debt
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
server::secret::Secret redacts Debug and zeroizes on drop, and the seam is the right one — expose() is deliberately loud and the type has no Serialize. But every consumer immediately derives a plain String that carries neither property:

- routes/settings.rs trimmed_new_password and the two `current` locals
- routes/status.rs, which passes a String into db::set_db_password

Those are the values that live across await points, move into spawn_blocking, and land in the process global. They are dropped without being zeroed, so the plaintext password sits in freed heap for the process lifetime — recoverable from a core dump, a swapped page, or a heap scan. zeroize is already a dependency; making trimmed_new_password return Zeroizing<String> and db::set_db_password take one closes every site without a new type.

Separately, session_token is the one credential Secret is not applied to. AppState derives Debug and holds it as a plain Arc<str>, so a single dbg! or a derived Debug on any future struct holding an AppState prints the whole session token — which is the entire authentication for the server. The comparison is already constant-time, so the threat model is understood; the Debug hole is the inconsistency.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Password strings derived from a Secret are zeroized on drop
- [ ] #2 session_token cannot be printed by a Debug impl
- [ ] #3 A test asserts a derived Debug of AppState contains no token
<!-- AC:END -->
