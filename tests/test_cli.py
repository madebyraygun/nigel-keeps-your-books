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
