---
id: TASK-68.3
title: Invoice template customization without rebuilding
status: In Progress
assignee:
  - '@opus-team'
created_date: '2026-08-08 00:28'
updated_date: '2026-08-08 00:50'
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Spec: docs/superpowers/specs/2026-08-08-task-68-3-template-override-design.md. Plan: docs/superpowers/plans/2026-08-08-task-68-3-template-override.md (10 TDD tasks).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan approved by orchestrator. Rulings on the spec's open questions:
1. Nested `invoice template export|path` accepted — a noun with two verbs earns the third level.
2. Vocabulary expansion (Tasks 7-8) is IN — 68.1 adds notes/terms and they must render; COMPANY fixes a real bug.
3. Default template gains NOTES/TERMS — required by 68.1's AC #4.
4. "from Raygun" subject fix folded in.
5. Ordering confirmed: 68.2 lands before 68.3.
6. PDF customization: to be filed as a separate subtask under the epic; shape decided there.
7. Unknown placeholders stay a hard load-time error — preview makes iteration cheap; a typo reaching a client is the worse failure.

Boundary vs 68.1 (ruled at plan review): {{NOTES}}/{{TERMS}} HTML rendering is 68.3's, the CLI flags and persistence are 68.1's. 68.1 confirmed both columns exist since v4.
<!-- SECTION:NOTES:END -->
