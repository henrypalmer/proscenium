"""Remove the player-e2e provider and restore the user's active provider."""

import os
import sqlite3

db = os.path.join(os.environ["APPDATA"], "proscenium", "proscenium.db")
con = sqlite3.connect(db)
con.execute("PRAGMA foreign_keys = ON")
con.execute("DELETE FROM providers WHERE name = 'E2E Player Test'")
for fts in ("fts_live_channels", "fts_movies", "fts_series"):
    con.execute(f"INSERT INTO {fts}({fts}) VALUES('rebuild')")
row = con.execute(
    "SELECT id, name FROM providers ORDER BY created_at LIMIT 1"
).fetchone()
if row:
    con.execute(
        "INSERT INTO settings (key, value) VALUES ('active_provider_id', ?)"
        " ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (row[0],),
    )
con.commit()
print("providers left:", [r[0] for r in con.execute("SELECT name FROM providers")])
print("active restored to:", row[1] if row else None)
