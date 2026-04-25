---
id: TASK-22
title: Add FAQ and documentation pages to the site
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels: []
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/169'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Add an FAQ page and a full documentation page to the GitHub Pages site at nigel.rygn.io. Ref #163 (initial site setup), PR #167 (site deployment).

## Detail

The site currently consists of a single landing page (`site/index.html`). It needs two additional pages:

* **FAQ page** — common questions about Nigel covering topics like supported bank formats, data portability, encryption, backup/restore, and differences from QuickBooks/other tools
* **Documentation page** — comprehensive reference documentation covering installation, configuration, all CLI commands, the dashboard, import workflow, rules engine, review process, reports, reconciliation, and settings
* Both pages should match the existing site design (dark terminal aesthetic, JetBrains Mono, teal accents, section labels)
* Navigation between pages needs to be added (header nav or footer links on all pages including `index.html`)
* The existing `docs/` directory in the repo contains `importers.md`, `walkthrough.md`, and `skills.md` which can inform documentation content
* The site is deployed via GitHub Actions from the `site/` folder

## Proposed solution

Create `site/faq.html` and `site/docs.html` using the same `styles.css` and visual language as the landing page. Add navigation links across all three pages. Documentation content should be derived from CLAUDE.md, README.md, and the `docs/` directory.

## Acceptance Criteria

- [ ] `site/faq.html` created with common questions and answers
- [ ] `site/docs.html` created with comprehensive usage documentation
- [ ] Navigation added to all site pages (index, FAQ, docs)
- [ ] Pages match existing site design and responsive behavior
- [ ] All internal and external links work correctly
- [ ] All linting checks pass
- [ ] Update test coverage
- [ ] Create or update documentation, making sure to remove any out of date information
- [ ] **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user

---
*Migrated from [GitHub issue #169](https://github.com/madebyraygun/nigel-keeps-your-books/issues/169)*
<!-- SECTION:DESCRIPTION:END -->
