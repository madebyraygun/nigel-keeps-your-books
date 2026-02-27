import csv
from pathlib import Path

from typer.testing import CliRunner

from nigel.cli import app
from nigel.models import ImporterInfo, ParsedRow
from nigel.importer import parse_amount, parse_date_mdy
from nigel.registry import registry

runner = CliRunner()


def _create_synthetic_csv(path: Path) -> None:
    """Write a minimal CSV that our test importer can parse."""
    with open(path, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["date", "description", "amount"])
        writer.writerow(["01/15/2025", "VENDOR A payment", "-500.00"])
        writer.writerow(["01/16/2025", "CLIENT B deposit", "2000.00"])
        writer.writerow(["01/20/2025", "VENDOR C charge", "-75.50"])


def _parse_synthetic(file_path: Path) -> list[ParsedRow]:
    rows = []
    with open(file_path, newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append(ParsedRow(
                date=parse_date_mdy(row["date"]),
                description=row["description"],
                amount=parse_amount(row["amount"]),
            ))
    return rows


# Register CLI test importer if not already registered
if registry.get_by_key("synthetic_test") is None:
    registry.register(ImporterInfo(
        key="synthetic_test", name="Synthetic Test",
        account_types=["synthetic"], file_extensions=[".csv"],
        parse=_parse_synthetic,
    ))


def _init(tmp_path, monkeypatch):
    """Point settings at tmp_path and run init with a custom data dir."""
    config_dir = tmp_path / "config"
    monkeypatch.setattr("nigel.settings.CONFIG_DIR", config_dir)
    monkeypatch.setattr("nigel.settings.SETTINGS_PATH", config_dir / "settings.json")
    data_dir = tmp_path / "data"
    result = runner.invoke(app, ["init", "--data-dir", str(data_dir)])
    assert result.exit_code == 0
    return data_dir


def test_init_creates_data_dir_and_db(tmp_path, monkeypatch):
    data_dir = _init(tmp_path, monkeypatch)
    assert (data_dir / "nigel.db").exists()
    assert (data_dir / "imports").is_dir()
    assert (data_dir / "exports").is_dir()


def test_init_writes_settings(tmp_path, monkeypatch):
    _init(tmp_path, monkeypatch)
    assert (tmp_path / "config" / "settings.json").exists()


def test_init_is_idempotent(tmp_path, monkeypatch):
    data_dir = _init(tmp_path, monkeypatch)
    result = runner.invoke(app, ["init", "--data-dir", str(data_dir)])
    assert result.exit_code == 0


def test_accounts_add_and_list(tmp_path, monkeypatch):
    _init(tmp_path, monkeypatch)

    result = runner.invoke(
        app,
        ["accounts", "add", "BofA Checking", "--type", "checking",
         "--institution", "Bank of America", "--last-four", "1234"],
    )
    assert result.exit_code == 0

    result = runner.invoke(app, ["accounts", "list"])
    assert result.exit_code == 0
    assert "BofA Checking" in result.output


def test_import_command(tmp_path, monkeypatch):
    _init(tmp_path, monkeypatch)
    runner.invoke(app, ["accounts", "add", "Test Acct", "--type", "synthetic"])

    csv_file = tmp_path / "test_import.csv"
    _create_synthetic_csv(csv_file)

    result = runner.invoke(
        app, ["import", str(csv_file), "--account", "Test Acct"]
    )
    assert result.exit_code == 0
    assert "3 imported" in result.output


def test_import_command_copies_file_to_imports(tmp_path, monkeypatch):
    data_dir = _init(tmp_path, monkeypatch)
    runner.invoke(app, ["accounts", "add", "Test Acct", "--type", "synthetic"])

    csv_file = tmp_path / "test_import.csv"
    _create_synthetic_csv(csv_file)

    runner.invoke(
        app, ["import", str(csv_file), "--account", "Test Acct"]
    )
    assert (data_dir / "imports" / "test_import.csv").exists()


def test_categorize_command(tmp_path, monkeypatch):
    _init(tmp_path, monkeypatch)
    runner.invoke(app, ["accounts", "add", "Test Acct", "--type", "synthetic"])

    csv_file = tmp_path / "test_import.csv"
    _create_synthetic_csv(csv_file)
    runner.invoke(app, ["import", str(csv_file), "--account", "Test Acct"])

    result = runner.invoke(app, ["categorize"])
    assert result.exit_code == 0
    assert "categorized" in result.output.lower() or "flagged" in result.output.lower()


def test_rules_add_and_list(tmp_path, monkeypatch):
    _init(tmp_path, monkeypatch)

    result = runner.invoke(
        app,
        ["rules", "add", "ADOBE", "--category", "Software & Subscriptions", "--vendor", "Adobe"],
    )
    assert result.exit_code == 0

    result = runner.invoke(app, ["rules", "list"])
    assert result.exit_code == 0
    assert "ADOBE" in result.output
