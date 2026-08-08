---
id: TASK-68.3
title: Invoice template customization without rebuilding
status: To Do
assignee: []
created_date: '2026-08-08 00:28'
labels:
  - invoicing
dependencies: []
parent_task_id: TASK-68
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
invoice.html is include_str!-compiled into the binary and the PDF layout is code in pdf.rs, so any styling change means editing src and rebuilding. Let the renderer look for <data_dir>/templates/invoice.html first and fall back to the embedded default, keeping the injection-safe single-pass {{KEY}} expansion. Document the full placeholder vocabulary in docs/invoicing.md, and add a command or flag that writes the embedded default out as a starting point. PDF customization can stay minimal (company block, logo) — the HTML page is what clients see first.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A template in the data dir overrides the embedded invoice.html; absence falls back to the default
- [ ] #2 The placeholder vocabulary is documented, and the default template can be exported as a starting point
- [ ] #3 Template expansion stays injection-safe with user-supplied templates
<!-- AC:END -->
