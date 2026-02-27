import json

from nigel.settings import load_settings, save_settings, get_data_dir, DEFAULTS


def test_save_and_load_roundtrip(tmp_path, monkeypatch):
    monkeypatch.setattr("nigel.settings.SETTINGS_PATH", tmp_path / "settings.json")
    monkeypatch.setattr("nigel.settings.CONFIG_DIR", tmp_path)
    data = {**DEFAULTS, "company_name": "Test Co"}
    save_settings(data)
    loaded = load_settings()
    assert loaded["company_name"] == "Test Co"


def test_load_settings_returns_defaults_when_missing(tmp_path, monkeypatch):
    monkeypatch.setattr("nigel.settings.SETTINGS_PATH", tmp_path / "settings.json")
    settings = load_settings()
    assert settings == DEFAULTS


def test_load_settings_merges_with_defaults(tmp_path, monkeypatch):
    settings_path = tmp_path / "settings.json"
    settings_path.write_text(json.dumps({"company_name": "Acme"}))
    monkeypatch.setattr("nigel.settings.SETTINGS_PATH", settings_path)
    settings = load_settings()
    assert settings["company_name"] == "Acme"
    assert settings["fiscal_year_start"] == "01"


def test_get_data_dir_reads_from_settings(tmp_path, monkeypatch):
    settings_path = tmp_path / "settings.json"
    settings_path.write_text(json.dumps({"data_dir": "/tmp/custom-books"}))
    monkeypatch.setattr("nigel.settings.SETTINGS_PATH", settings_path)
    assert str(get_data_dir()) == "/tmp/custom-books"


def test_save_creates_config_dir(tmp_path, monkeypatch):
    config_dir = tmp_path / "config" / "nigel"
    monkeypatch.setattr("nigel.settings.CONFIG_DIR", config_dir)
    monkeypatch.setattr("nigel.settings.SETTINGS_PATH", config_dir / "settings.json")
    save_settings(DEFAULTS)
    assert config_dir.exists()
    assert (config_dir / "settings.json").exists()
