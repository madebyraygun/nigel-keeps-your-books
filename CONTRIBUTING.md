# Contributing

## Setup

```bash
git clone https://github.com/madebyraygun/nigel-keeps-your-books.git
cd nigel-keeps-your-books
uv sync
```

Install first-party plugins for local development:

```bash
uv pip install -e plugins/nigel-bofa
uv pip install -e plugins/nigel-gusto
uv pip install -e plugins/nigel-k1
uv pip install -e plugins/nigel-export-pdf
```

## Running Tests

```bash
uv run pytest -v                    # Core tests (56)
cd plugins/nigel-bofa && uv run pytest -v    # BofA plugin tests
cd plugins/nigel-gusto && uv run pytest -v   # Gusto plugin tests
cd plugins/nigel-k1 && uv run pytest -v      # K-1 plugin tests
cd plugins/nigel-export-pdf && uv run pytest -v  # PDF export tests
```

Core tests use synthetic fixtures — they don't depend on any plugin being installed.

## Project Layout

```
src/nigel/          # Core package
plugins/            # First-party plugin packages
  nigel-bofa/       # Bank of America importers
  nigel-gusto/      # Gusto payroll importer
  nigel-k1/         # K-1 prep report
  nigel-export-pdf/ # PDF export
tests/              # Core tests
docs/               # Walkthrough and documentation
```

## Writing a Plugin

Plugins are Python packages that register via the `nigel.plugins` entry point. A plugin exposes a `register(hooks, **kwargs)` function that receives a `PluginHooks` object.

A minimal importer plugin:

```python
# my_plugin/plugin.py
from nigel.models import ImporterInfo, ParsedRow
from nigel.plugins import PluginHooks

def detect(file_path):
    """Return True if this file matches your format."""
    with open(file_path) as f:
        return "MY HEADER" in f.readline()

def parse(file_path):
    """Parse the file into a list of ParsedRow."""
    rows = []
    # ... your parsing logic ...
    return rows

def register(hooks: PluginHooks, **kwargs):
    hooks.importers.append(
        ImporterInfo(
            key="my_format",
            label="My Bank Checking",
            account_type="checking",
            file_type="csv",
            institution="My Bank",
            parse=parse,
            detect=detect,
        )
    )
```

Wire it up in `pyproject.toml`:

```toml
[project.entry-points."nigel.plugins"]
my_bank = "my_plugin.plugin"
```

Plugins can also:

- Add CLI subcommands via `hooks.cli_commands`
- Run database migrations via `hooks.migrations`
- Seed categories via `hooks.category_seeds`

See the existing plugins in `plugins/` for working examples.

## Code Style

- Python 3.12+
- No external linter config — keep it clean and consistent with surrounding code
- Prefer simple, flat code over abstractions
- Cash amounts are plain floats — negative = expense, positive = income

## Documentation

Every feature change should update:

- **CLAUDE.md** — architecture, commands, structure, and constraints sections
- **README.md** — features, quick start, and plugin table if applicable

## Commits

- Keep commits focused — one logical change per commit
- Use imperative mood in commit messages ("Add feature" not "Added feature")
