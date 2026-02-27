from nigel_k1.migrations import create_k1_tables
from nigel_k1.categories import K1_CATEGORIES


def register(hooks, app=None, report_app=None, **kwargs):
    hooks.add_migration(create_k1_tables)
    hooks.add_categories(K1_CATEGORIES)
