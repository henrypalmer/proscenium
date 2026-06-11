"""Print catalog row counts and refresh state of the live app database."""

import os
import sqlite3

db = os.path.join(os.environ["APPDATA"], "proscenium", "proscenium.db")
con = sqlite3.connect(db)
for table in (
    "live_channels",
    "live_categories",
    "movies",
    "vod_categories",
    "series",
    "series_categories",
    "episodes",
):
    n = con.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
    print(f"{table}: {n}")
row = con.execute("SELECT name, last_refreshed FROM providers").fetchone()
print(f"provider: {row[0]}, last_refreshed: {row[1]}")
fts = con.execute(
    "SELECT name FROM fts_live_channels WHERE fts_live_channels MATCH 'seed' LIMIT 1"
).fetchone()
print(f"fts sample hit: {fts[0] if fts else None}")
