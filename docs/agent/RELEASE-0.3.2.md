MegaRustDesk 0.3.2 preview keeps one main portable instance per Windows user and session.

Launching the main app closes older verified copies, including copies in other folders
and legacy portable host/tray processes. The new package is validated before takeover.
Previous active connections may close with the old main process and its Windows job.

External MCP clients, independent viewer windows, update helpers, unknown modes,
other users/sessions and unrelated products are excluded from direct termination.
An existing executable must match its manifest SHA-256. Unverified copies require
manual closure. A launch descended from the old host is refused to avoid losing the new
process when the old kill-on-close job ends.

Same-folder, cross-folder, parallel launch, worker preservation, synthetic data,
hash mismatch and parent-host cases are checked with native Windows processes.
This preview does not by itself confirm a working connection to the second physical PC.
Sciter browser gate and clean Windows VM acceptance remain unverified.
