from pathlib import Path

from nigel.models import ImporterInfo, ParsedRow
from nigel.registry import ImporterRegistry


def _dummy_parse(path: Path) -> list[ParsedRow]:
    return [ParsedRow(date="2025-01-01", description="Test", amount=-10.0)]


def _dummy_detect(path: Path) -> bool:
    return path.suffix == ".csv"


def _dummy_post_import(conn, account_id, rows):
    pass


def _dummy_parser(path):
    return []


def test_register_and_get_by_account_type():
    reg = ImporterRegistry()
    info = ImporterInfo(
        key="test_checking", name="Test Checking",
        account_types=["checking"], file_extensions=[".csv"],
        parse=_dummy_parser,
    )
    reg.register(info)
    result = reg.get_for_account_type("checking")
    assert result is not None
    assert result.key == "test_checking"


def test_get_for_unknown_account_type_returns_none():
    reg = ImporterRegistry()
    assert reg.get_for_account_type("unknown") is None


def test_list_all():
    reg = ImporterRegistry()
    info = ImporterInfo(
        key="test_cc", name="Test CC",
        account_types=["credit_card"], file_extensions=[".csv"],
        parse=_dummy_parser,
    )
    reg.register(info)
    assert len(reg.list_all()) == 1


def test_register_multiple_account_types():
    reg = ImporterRegistry()
    info = ImporterInfo(
        key="multi", name="Multi Parser",
        account_types=["checking", "savings"], file_extensions=[".csv"],
        parse=_dummy_parser,
    )
    reg.register(info)
    assert reg.get_for_account_type("checking").key == "multi"
    assert reg.get_for_account_type("savings").key == "multi"


def test_importer_info_has_version_detect_post_import():
    info = ImporterInfo(
        key="test", name="Test", account_types=["checking"],
        file_extensions=[".csv"], parse=_dummy_parse,
        version="2.0", detect=_dummy_detect, post_import=_dummy_post_import,
    )
    assert info.version == "2.0"
    assert info.detect is _dummy_detect
    assert info.post_import is _dummy_post_import


def test_importer_info_defaults():
    info = ImporterInfo(
        key="test", name="Test", account_types=["checking"],
        file_extensions=[".csv"], parse=_dummy_parse,
    )
    assert info.version == "1.0"
    assert info.detect is None
    assert info.post_import is None


def test_get_by_key():
    reg = ImporterRegistry()
    info = ImporterInfo(
        key="bofa_checking", name="BofA Checking",
        account_types=["checking"], file_extensions=[".csv"],
        parse=_dummy_parse,
    )
    reg.register(info)
    assert reg.get_by_key("bofa_checking") is info
    assert reg.get_by_key("nonexistent") is None


def test_get_for_file_uses_detect(tmp_path):
    reg = ImporterRegistry()
    csv_file = tmp_path / "test.csv"
    csv_file.write_text("header\nrow")

    info_with_detect = ImporterInfo(
        key="smart", name="Smart", account_types=["checking"],
        file_extensions=[".csv"], parse=_dummy_parse,
        detect=_dummy_detect,
    )
    reg.register(info_with_detect)
    assert reg.get_for_file("checking", csv_file) is info_with_detect


def test_get_for_file_falls_back_without_detect(tmp_path):
    reg = ImporterRegistry()
    csv_file = tmp_path / "test.csv"
    csv_file.write_text("header\nrow")

    info_no_detect = ImporterInfo(
        key="basic", name="Basic", account_types=["checking"],
        file_extensions=[".csv"], parse=_dummy_parse,
    )
    reg.register(info_no_detect)
    assert reg.get_for_file("checking", csv_file) is info_no_detect


def test_public_helpers():
    from nigel.importer import parse_amount, parse_date_mdy, excel_serial_to_date
    assert parse_amount("1,234.56") == 1234.56
    assert parse_date_mdy("01/15/2025") == "2025-01-15"
    assert excel_serial_to_date(45667) == "2025-01-10"
