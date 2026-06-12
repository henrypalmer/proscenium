"""Point the active-provider setting back at the user's real provider."""

import os
import sqlite3

db = os.path.join(os.environ["APPDATA"], "proscenium", "proscenium.db")
con = sqlite3.connect(db)
row = con.execute("SELECT id, name FROM providers ORDER BY created_at LIMIT 1").fetchone()
con.execute(
    "INSERT INTO settings (key, value) VALUES ('active_provider_id', ?)"
    " ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    (row[0],),
)
con.commit()
print(f"active provider restored to: {row[1]} ({row[0]})")
