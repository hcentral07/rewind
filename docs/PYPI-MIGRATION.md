# Retiring the Python `rwnd` package

`rwnd` 0.1 (the Python port) records a SHA-256 of each file in SQLite **but never stores the file contents**, so it cannot restore anything. People who
installed it deserve to know and to be pointed at the Rust version, which stores contents and does everything in the README.

Suggested plan: publish one last `rwnd` release that prints a clear message instead of pretending to work, then stop.

## The final release

Keep your existing `pyproject.toml` (it is not in the repo zip) and change:

- version to `0.2.0`
- remove the `watchdog` and `rich` dependencies
- classifier `Development Status :: 7 - Inactive`
- description: "Moved: Rewind is now a Rust app. See github.com/hcentral07/rewind"

and replace the CLI module with:

```python
import sys

MESSAGE = """
rwnd has moved.

This Python prototype only stored file hashes and could not restore files.
Rewind is now a Rust program for Windows and Linux (file contents, folder rewind,
undo for AI agents, ransomware tripwire, plugins).

Install on Linux:
    curl -fsSL https://raw.githubusercontent.com/hcentral07/rewind/main/scripts/install.sh | sh
Everything else: https://github.com/hcentral07/rewind
"""


def main():
    print(MESSAGE, file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

Then `python -m build` and `twine upload dist/*`. Do not delete (yank) the old versions: people pinning them should still be able to install, and the
README on PyPI should carry the same message.

## Old data
`~/.config/rewind/rewind.db` from `rwnd` holds only paths and hashes; there is nothing to migrate. Rust `rewind` uses `~/.local/share/rewind`.
