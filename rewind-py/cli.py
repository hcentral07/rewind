import argparse
from pathlib import Path
from rich.console import Console
from rich.table import Table
from .core import start_watcher, get_db

console = Console()

def main():
    parser = argparse.ArgumentParser(description="rewind - A passive time machine for your filesystem.")
    subparsers = parser.add_subparsers(dest="command")

    watch_parser = subparsers.add_parser("watch", help="Watch a folder for changes")
    watch_parser.add_argument("path", help="Path to the folder to watch")

    log_parser = subparsers.add_parser("log", help="View version history of a file")
    log_parser.add_argument("file", help="Path to the file")

    args = parser.parse_args()

    if args.command == "watch":
        start_watcher(args.path)
    
    elif args.command == "log":
        conn = get_db()
        filepath = str(Path(args.file).resolve())
        rows = conn.execute("SELECT * FROM versions WHERE path = ? ORDER BY id DESC", (filepath,)).fetchall()
        
        if not rows:
            console.print(f"[red]No history found for {filepath}[/red]")
            return

        table = Table(title=f"History for {args.file}")
        table.add_column("Version", style="cyan")
        table.add_column("Hash", style="magenta")
        table.add_column("Date", style="green")
        
        for row in rows:
            table.add_row(str(row["id"]), row["hash"][:8], row["created_at"])
        console.print(table)

    else:
        parser.print_help()

if __name__ == "__main__":
    main()
