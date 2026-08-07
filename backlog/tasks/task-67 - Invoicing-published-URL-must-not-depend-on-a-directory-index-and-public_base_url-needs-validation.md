---
id: TASK-67
title: >-
  Invoicing: published URL must not depend on a directory index, and
  public_base_url needs validation
status: To Do
assignee: []
created_date: '2026-08-07 23:10'
labels:
  - invoicing
  - bug
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/pull/172'
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two related failures from a real send against billing.rygn.io:

1. Nigel prints and emails {public_base_url}/{token}/, but an R2 custom domain does not serve index documents, so the directory URL 404s while .../{token}/index.html works. Either emit the full index.html URL everywhere the link is printed, emailed, or attached to the Stripe link (robust on any static host), or document that the docs' "served at .../{token}/" claim requires an edge rewrite (Cloudflare rule/Worker appending index.html) and make that a setup step in docs/invoicing.md.

2. public_base_url accepted "billing.rygn.io" (no scheme, no /i prefix) silently, producing broken links in a real email. Validate at send time: require an http(s) scheme, and warn when the path does not end in the /i prefix Nigel writes keys under.

Found during pre-merge testing of PR #172.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The link printed by send and embedded in the email resolves on a plain R2 custom domain with no edge rewrite, or the rewrite requirement is documented as a required setup step
- [ ] #2 A public_base_url without an http(s) scheme is rejected by name before anything is published or emailed
- [ ] #3 A public_base_url whose path does not end in /i produces a warning
<!-- AC:END -->
