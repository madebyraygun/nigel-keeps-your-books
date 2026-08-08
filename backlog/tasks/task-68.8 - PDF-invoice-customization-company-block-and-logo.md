---
id: TASK-68.8
title: 'PDF invoice customization: company block and logo'
status: To Do
assignee: []
created_date: '2026-08-08 01:02'
labels:
  - invoicing
dependencies: []
parent_task_id: TASK-68
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Deferred out of 68.3 by design review: pdf.rs has no template, only imperative layout, so "customizable" means either a structured settings shape (company block, typography, logo path — with its own validation and export) or an HTML-to-PDF dependency decision (headless browser / Typst / Weasyprint), which carries real weight in a single-static-binary tool. This task owns that decision. Minimum bar: company name from metadata renders in the PDF header (parity with the HTML {{COMPANY}} from 68.3); logo support decided here.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The PDF carries the operator's company identity without rebuilding the binary
- [ ] #2 The customization mechanism's shape (settings vs HTML-to-PDF) is decided and documented
<!-- AC:END -->
