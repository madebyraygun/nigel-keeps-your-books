---
# nigel-to27
title: Plugin system for extending Nigel
status: completed
type: feature
priority: high
created_at: 2026-02-27T00:56:17Z
updated_at: 2026-02-27T01:35:07Z
blocked_by:
    - nigel-r3cb
---

Design a plugin system that lets external packages hook new functionality into Nigel — importers, reports, CLI commands, and DB schema extensions.

## Motivation
Features like the K-1 prep report (see K-1 addendum plan) require new DB tables (entity_config, shareholders), new categories, new report commands, and new CLI subcommands. These shouldn't live in core — they should be installable plugins.

## Requirements
- Plugin discovery via Python entry points (e.g. `[project.entry-points."nigel.plugins"]`)
- Hook points: importers, reports, CLI commands, DB migrations, category seeds
- Plugins can register new Typer subcommands on the main app
- Plugins can extend the DB schema (migration hooks run on `nigel init`)
- Plugins can register new importer parsers (builds on the importer schema work)
- Simple plugin API — a plugin is a Python package that exposes a `register(app)` function or similar

## Example Plugin
The K-1 prep report would be a plugin that:
- Adds `entity_config` and `shareholders` tables
- Adds new categories (Charitable Contributions, Section 179 Equipment, Officer Compensation)
- Registers `nigel report k1-prep` command
- Extends settings.json with entity configuration

## Out of Scope
- Runtime plugin management CLI (install/uninstall) — just use pip/uv
- Plugin marketplace or registry
