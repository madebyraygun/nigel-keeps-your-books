---
# nigel-r3cb
title: Importer schema and registration mechanism
status: todo
type: feature
priority: high
created_at: 2026-02-27T00:56:06Z
updated_at: 2026-02-27T00:56:06Z
---

Design a flexible importer system that allows new file formats to be added without modifying core code.

## Current State
The importer (`src/nigel/importer.py`) has hardcoded parsers mapped via `PARSER_MAP` dict keyed by account type (checking, credit_card, line_of_credit, payroll). Adding a new bank or format means editing `importer.py` directly.

## Requirements
- Define a standard importer schema/interface that all parsers must implement (input file → list of ParsedRow)
- Registration mechanism so parsers can be discovered and registered (decorator, entry points, or config-based)
- Parser selection should support both account type and auto-detection (file content sniffing)
- Preserve backward compatibility with existing BofA and Gusto parsers
- Importers should be able to declare metadata: supported file extensions, institution name, account types

## Out of Scope
- Plugin loading from external packages (that's the plugin system issue)
- UI for managing importers
