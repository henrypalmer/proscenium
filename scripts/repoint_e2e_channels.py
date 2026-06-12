"""Point the e2e test channels at the MPEG-TS stream asset."""

import os
import sqlite3

db = os.path.join(os.environ["APPDATA"], "proscenium", "proscenium.db")
con = sqlite3.connect(db)
con.execute(
    "UPDATE live_channels SET stream_url = 'http://127.0.0.1:8765/test-h264.ts',"
    " stream_ext = 'ts' WHERE id LIKE 'e2e-%'"
)
con.commit()
print("channels updated:", con.total_changes)
