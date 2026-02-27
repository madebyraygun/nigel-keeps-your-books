from nigel.models import ImporterInfo
from nigel_bofa.checking import parse_checking, detect_checking
from nigel_bofa.credit_card import parse_credit_card, detect_credit_card
from nigel_bofa.line_of_credit import parse_line_of_credit, detect_line_of_credit


def register(hooks, **kwargs):
    hooks.add_importer(ImporterInfo(
        key="bofa_checking", name="Bank of America Checking",
        account_types=["checking"], file_extensions=[".csv"],
        parse=parse_checking, detect=detect_checking,
    ))
    hooks.add_importer(ImporterInfo(
        key="bofa_credit_card", name="Bank of America Credit Card",
        account_types=["credit_card"], file_extensions=[".csv"],
        parse=parse_credit_card, detect=detect_credit_card,
    ))
    hooks.add_importer(ImporterInfo(
        key="bofa_line_of_credit", name="Bank of America Line of Credit",
        account_types=["line_of_credit"], file_extensions=[".csv"],
        parse=parse_line_of_credit, detect=detect_line_of_credit,
    ))
