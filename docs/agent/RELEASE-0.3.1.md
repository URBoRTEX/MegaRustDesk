MegaRustDesk 0.3.1 preview fixes Russian text and detects incomplete manual extraction.

- Explicit UTF-8 document charset and locale-independent build resource reads.
- GUI startup checks the compiled version and EXE/Sciter hashes against manifest.json.
- A mixed installation fails with an actionable message before the receiver starts.
- Existing data and external Codex settings remain unchanged.

Connection diagnosis is still separate: a relay TCP connection alone does not prove
the remote session is ready. See docs/agent/STATUS.md for verified scope and limitations.
This preview is excluded from automatic stable-release discovery.
