import pytest
from nigel.db import get_connection, init_db


@pytest.fixture
def db(tmp_path):
    db_path = tmp_path / "test.db"
    conn = get_connection(db_path)
    init_db(conn)
    yield conn
    conn.close()
