from pathlib import Path

from nigel.models import ImporterInfo


class ImporterRegistry:
    def __init__(self):
        self._importers: dict[str, ImporterInfo] = {}
        self._by_account_type: dict[str, list[ImporterInfo]] = {}

    def register(self, info: ImporterInfo) -> None:
        self._importers[info.key] = info
        for acct_type in info.account_types:
            self._by_account_type.setdefault(acct_type, []).append(info)

    def get_by_key(self, key: str) -> ImporterInfo | None:
        return self._importers.get(key)

    def get_for_account_type(self, account_type: str) -> ImporterInfo | None:
        importers = self._by_account_type.get(account_type, [])
        return importers[0] if importers else None

    def get_for_file(self, account_type: str, file_path: Path) -> ImporterInfo | None:
        importers = self._by_account_type.get(account_type, [])
        for imp in importers:
            if imp.detect and imp.detect(file_path):
                return imp
        return importers[0] if importers else None

    def list_all(self) -> list[ImporterInfo]:
        return list(self._importers.values())


registry = ImporterRegistry()
