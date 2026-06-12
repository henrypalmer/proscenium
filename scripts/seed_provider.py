"""Seed the live app database with a local M3U provider for manual testing.

Generates a playlist in %TEMP%, registers it as a provider with a NULL
last_refreshed (stale), and marks it active — so the next app launch
exercises the startup background-refresh path end to end.
"""

import os
import sqlite3
import tempfile
import time
import uuid

playlist = os.path.join(tempfile.gettempdir(), "proscenium-seed.m3u")
with open(playlist, "w", encoding="utf-8") as f:
    f.write("#EXTM3U\n")
    for i in range(12000):
        f.write(
            f'#EXTINF:-1 tvg-id="seed{i}" group-title="Group {i % 25}",Seed Channel {i}\n'
        )
        f.write(f"http://stream.invalid/live/{i}.ts\n")
    for i in range(1500):
        f.write(
            f'#EXTINF:-1 group-title="VOD | Genre {i % 10}",Seed Movie {i} (20{i % 25:02d})\n'
        )
        f.write(f"http://stream.invalid/vod/{i}.mp4\n")
    for i in range(50):
        for ep in range(1, 6):
            f.write(
                f'#EXTINF:-1 group-title="Series | Drama",Seed Show {i} S01E{ep:02d}\n'
            )
            f.write(f"http://stream.invalid/series/{i}-{ep}.mp4\n")

db = os.path.join(os.environ["APPDATA"], "proscenium", "proscenium.db")
con = sqlite3.connect(db)
provider_id = str(uuid.uuid4())
con.execute(
    "INSERT INTO providers (id, name, type, playlist_url, local_file_path, last_refreshed, created_at)"
    " VALUES (?, 'Seed Playlist', 'm3u', NULL, ?, NULL, ?)",
    (provider_id, playlist, int(time.time())),
)
con.execute(
    "INSERT INTO settings (key, value) VALUES ('active_provider_id', ?)"
    " ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    (provider_id,),
)
con.commit()
print(f"seeded provider {provider_id} -> {playlist}")
