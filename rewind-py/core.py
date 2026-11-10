import hashlib
import sqlite3
import os
import time
from pathlib import Path
from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler

DB_PATH = Path.home() / ".config" / "rewind" / "rewind.db"

def get_db():
    DB_PATH.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    conn.execute('''
        CREATE TABLE IF NOT EXISTS versions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            hash TEXT NOT NULL,
            created_at TEXT NOT NULL
        )
    ''')
    return conn

def hash_file(filepath):
    hasher = hashlib.sha256()
    try:
        with open(filepath, 'rb') as f:
            for chunk in iter(lambda: f.read(4096), b""):
                hasher.update(chunk)
        return hasher.hexdigest()
    except (PermissionError, FileNotFoundError):
        return None

def save_version(filepath):
    filepath = str(Path(filepath).resolve())
    file_hash = hash_file(filepath)
    if not file_hash:
        return

    conn = get_db()
    last = conn.execute("SELECT hash FROM versions WHERE path = ? ORDER BY id DESC LIMIT 1", (filepath,)).fetchone()
    
    if last and last["hash"] == file_hash:
        return

    conn.execute(
        "INSERT INTO versions (path, hash, created_at) VALUES (?, ?, datetime('now'))",
        (filepath, file_hash)
    )
    conn.commit()
    print(f"[rewind] Saved: {filepath}")

class RewindHandler(FileSystemEventHandler):
    def on_modified(self, event):
        if not event.is_directory:
            save_version(event.src_path)

def start_watcher(path):
    path = str(Path(path).resolve())
    print(f"[rewind] Watching {path}... (Ctrl+C to stop)")
    
    for root, _, files in os.walk(path):
        for file in files:
            if file.startswith('.'): continue
            save_version(os.path.join(root, file))

    observer = Observer()
    observer.schedule(RewindHandler(), path, recursive=True)
    observer.start()
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        observer.stop()
    observer.join()
