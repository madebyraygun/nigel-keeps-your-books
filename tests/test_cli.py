from typer.testing import CliRunner

from bookkeeper.cli import app

runner = CliRunner()


def test_init_creates_data_dir_and_db(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))

    result = runner.invoke(app, ["init"])
    assert result.exit_code == 0
    assert (data_dir / "raygun.db").exists()
    assert (data_dir / "imports").is_dir()
    assert (data_dir / "exports").is_dir()


def test_init_is_idempotent(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))

    result1 = runner.invoke(app, ["init"])
    result2 = runner.invoke(app, ["init"])
    assert result1.exit_code == 0
    assert result2.exit_code == 0


def test_accounts_add_and_list(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))
    runner.invoke(app, ["init"])

    result = runner.invoke(
        app,
        ["accounts", "add", "BofA Checking", "--type", "checking",
         "--institution", "Bank of America", "--last-four", "1234"],
    )
    assert result.exit_code == 0

    result = runner.invoke(app, ["accounts", "list"])
    assert result.exit_code == 0
    assert "BofA Checking" in result.output


from pathlib import Path

FIXTURES = Path(__file__).parent / "fixtures"


def test_import_command(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))
    runner.invoke(app, ["init"])
    runner.invoke(app, ["accounts", "add", "BofA Checking", "--type", "checking"])

    result = runner.invoke(
        app, ["import", str(FIXTURES / "bofa_checking_sample.csv"), "--account", "BofA Checking"]
    )
    assert result.exit_code == 0
    assert "5 imported" in result.output


def test_import_command_copies_file_to_imports(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))
    runner.invoke(app, ["init"])
    runner.invoke(app, ["accounts", "add", "BofA Checking", "--type", "checking"])

    runner.invoke(
        app, ["import", str(FIXTURES / "bofa_checking_sample.csv"), "--account", "BofA Checking"]
    )
    assert (data_dir / "imports" / "bofa_checking_sample.csv").exists()


def test_categorize_command(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))
    runner.invoke(app, ["init"])
    runner.invoke(app, ["accounts", "add", "BofA Checking", "--type", "checking"])
    runner.invoke(app, ["import", str(FIXTURES / "bofa_checking_sample.csv"), "--account", "BofA Checking"])

    result = runner.invoke(app, ["categorize"])
    assert result.exit_code == 0
    assert "categorized" in result.output.lower() or "flagged" in result.output.lower()


def test_rules_add_and_list(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))
    runner.invoke(app, ["init"])

    result = runner.invoke(
        app,
        ["rules", "add", "ADOBE", "--category", "Software & Subscriptions", "--vendor", "Adobe"],
    )
    assert result.exit_code == 0

    result = runner.invoke(app, ["rules", "list"])
    assert result.exit_code == 0
    assert "ADOBE" in result.output
