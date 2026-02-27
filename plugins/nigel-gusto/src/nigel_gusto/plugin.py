from nigel.models import ImporterInfo
from nigel_gusto.payroll import parse_payroll, detect_payroll
from nigel_gusto.categorizer import auto_categorize_payroll


def register(hooks, **kwargs):
    hooks.add_importer(ImporterInfo(
        key="gusto_payroll", name="Gusto Payroll",
        account_types=["payroll"], file_extensions=[".xlsx"],
        parse=parse_payroll, detect=detect_payroll,
        post_import=auto_categorize_payroll,
    ))
