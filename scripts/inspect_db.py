"""Print the table/index inventory of the live app database (dev helper)."""

import os
import sqlite3

db = os.path.join(os.environ["APPDATA"], "proscenium", "proscenium.db")
con = sqlite3.connect(db)
tables = sorted(
    r[0]
    for r in con.execute("SELECT name FROM sqlite_master WHERE type = 'table'")
    if not r[0].startswith("fts_") and not r[0].startswith("sqlite_")
)
fts = sorted(
    r[0]
    for r in con.execute(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'fts!_%' ESCAPE '!'"
    )
    if "_" not in r[0].replace("fts_", "", 1) or True
)
indexes = sorted(
    r[0]
    for r in con.execute(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx%'"
    )
)
print("tables:", tables)
print("fts:", [t for t in fts if not any(t.endswith(s) for s in ("_data", "_idx", "_docsize", "_config", "_content"))])
print("indexes:", indexes)
print("providers rows:", con.execute("SELECT COUNT(*) FROM providers").fetchone()[0])
