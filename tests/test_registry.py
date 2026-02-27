from nigel.registry import ImporterRegistry
from nigel.models import ImporterInfo


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
