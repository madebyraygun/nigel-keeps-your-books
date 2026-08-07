---
id: TASK-58
title: Accessibility pass: focus management, live regions, field errors
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - accessibility
  - spa
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A batch of issues axe cannot see, found while reviewing task 31. The axe-over-every-preview-state claim holds; these are behavioural.

Focus is dropped to document.body on: register edit commit and cancel; manager dialog close in the accounts, categories and rules screens (the screens render wc-manager-dialog already open, so wa-dialog never captures originalTrigger); and confirmDialog(), which removes the element synchronously.

wc-register-table: Enter-to-edit never moves focus into the editor, which also makes the documented Esc-cancels inert; and the f binding has no modifier guard, so Cmd+F or Ctrl+F flags the focused transaction — a real PATCH — and preventDefault swallows the browser find bar.

wc-period-nav: arrow keys change the checked radio without moving focus, so the granularity change is never announced and the focus ring strands on the old button.

Live regions mounted together with their content, so a screen reader never announces them: wc-spinner (every call site mounts it conditionally), wc-dropzone, wc-category-picker. wc-toast is the one place that gets this right and is the pattern to copy.

Field errors on wa-input forms are unassociated across wc-reconcile-form, wc-account-form, wc-category-form, wc-rule-form and wc-import-form. Web Awesome's hint slot is already used correctly for non-error text nearby.

wc-review-form validation failure is silent: no role=alert, no focus move, and the error cannot be wired via aria-describedby because the input is inside wc-category-picker's shadow root.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Focus returns to a sensible element after every dialog, editor and confirm
- [ ] #2 Cmd+F and Ctrl+F reach the browser rather than flagging a transaction
- [ ] #3 Live regions are mounted before the text they announce
- [ ] #4 Field errors are associated with their inputs across every form
<!-- AC:END -->
