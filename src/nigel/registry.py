from nigel.models import ImporterInfo


class ImporterRegistry:
    def __init__(self):
        self._importers: dict[str, ImporterInfo] = {}
        self._by_account_type: dict[str, ImporterInfo] = {}

    def register(self, info: ImporterInfo) -> None:
        self._importers[info.key] = info
        for acct_type in info.account_types:
            self._by_account_type[acct_type] = info

    def get_for_account_type(self, account_type: str) -> ImporterInfo | None:
        return self._by_account_type.get(account_type)

    def list_all(self) -> list[ImporterInfo]:
        return list(self._importers.values())


registry = ImporterRegistry()
