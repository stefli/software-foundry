
import os
import sqlite3

print("Running Foundry Sandbox Smoke Test...")
db_path = os.environ.get("SQLITE_DATABASE_PATH", "test.db")
print(f"Connecting to isolated SQLite DB at: {db_path}")

conn = sqlite3.connect(db_path)
cursor = conn.cursor()
cursor.execute("CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, content TEXT);")
cursor.execute("INSERT INTO notes (content) VALUES ('Foundry self-test entry');")
conn.commit()

cursor.execute("SELECT COUNT(*) FROM notes;")
count = cursor.fetchone()[0]
print(f"SQLite verification success! Note count: {count}")
conn.close()
