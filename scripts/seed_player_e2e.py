"""Seed the live database for the player end-to-end test: a provider whose
channels point at a locally served test video. last_refreshed is set to now
so the startup stale check does not overwrite the seeded rows."""

import os
import sqlite3
import time
import uuid

db = os.path.join(os.environ["APPDATA"], "proscenium", "proscenium.db")
con = sqlite3.connect(db)
con.execute("PRAGMA foreign_keys = ON")
provider_id = str(uuid.uuid4())
now = int(time.time())
con.execute(
    "INSERT INTO providers (id, name, type, playlist_url, last_refreshed, created_at)"
    " VALUES (?, 'E2E Player Test', 'm3u', 'http://127.0.0.1:1/unused.m3u', ?, ?)",
    (provider_id, now, now),
)
con.execute(
    "INSERT INTO live_categories (id, provider_id, name, sort_order) VALUES ('Test', ?, 'Test', 0)",
    (provider_id,),
)
for i in range(3):
    con.execute(
        "INSERT INTO live_channels (id, provider_id, name, category_id, category_name,"
        " stream_url, stream_ext) VALUES (?, ?, ?, 'Test', 'Test', ?, 'mp4')",
        (
            f"e2e-{i}",
            provider_id,
            f"E2E Channel {i}",
            "http://127.0.0.1:8765/test-h264.mp4",
        ),
    )
con.execute(
    "INSERT INTO settings (key, value) VALUES ('active_provider_id', ?)"
    " ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    (provider_id,),
)
con.commit()
print(provider_id)
