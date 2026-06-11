"""List provider rows and the active-provider setting of the live database."""

import os
import sqlite3

db = os.path.join(os.environ["APPDATA"], "proscenium", "proscenium.db")
con = sqlite3.connect(db)
for row in con.execute(
    "SELECT id, name, type, server_url, playlist_url, local_file_path, last_refreshed FROM providers"
):
    print(row)
active = con.execute(
    "SELECT value FROM settings WHERE key = 'active_provider_id'"
).fetchone()
print("active:", active[0] if active else None)
