"""Remove the seed provider created by seed_provider.py and restore the
pre-seed state (no active provider selected; FTS re-synced)."""

import os
import sqlite3
import tempfile

db = os.path.join(os.environ["APPDATA"], "proscenium", "proscenium.db")
con = sqlite3.connect(db)
con.execute("PRAGMA foreign_keys = ON")
con.execute("DELETE FROM providers WHERE name = 'Seed Playlist'")
con.execute("DELETE FROM settings WHERE key = 'active_provider_id'")
for fts in ("fts_live_channels", "fts_movies", "fts_series"):
    con.execute(f"INSERT INTO {fts}({fts}) VALUES('rebuild')")
con.commit()

playlist = os.path.join(tempfile.gettempdir(), "proscenium-seed.m3u")
if os.path.exists(playlist):
    os.remove(playlist)

for row in con.execute("SELECT name FROM providers"):
    print("remaining provider:", row[0])
print(
    "live_channels:",
    con.execute("SELECT COUNT(*) FROM live_channels").fetchone()[0],
)
