import json
from pathlib import Path

CONFIG_DIR = Path.home() / ".config" / "nigel"
SETTINGS_PATH = CONFIG_DIR / "settings.json"

DEFAULT_DATA_DIR = Path.home() / "Documents" / "nigel"

DEFAULTS = {
    "data_dir": str(DEFAULT_DATA_DIR),
    "company_name": "",
    "fiscal_year_start": "01",
}


def load_settings() -> dict:
    if SETTINGS_PATH.exists():
        with open(SETTINGS_PATH) as f:
            saved = json.loads(f.read())
        return {**DEFAULTS, **saved}
    return dict(DEFAULTS)


def save_settings(settings: dict) -> None:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    with open(SETTINGS_PATH, "w") as f:
        f.write(json.dumps(settings, indent=2) + "\n")


def get_data_dir() -> Path:
    return Path(load_settings()["data_dir"])
