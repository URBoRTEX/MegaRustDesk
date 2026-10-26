MegaRustDesk 0.3.0 preview adds manual signed updates to the Windows x64 portable client.

- External Codex CLI/MCP; no embedded chat or Codex runtime.
- Check, download and apply from the main window; public GitHub assets need no client token.
- Ed25519 signed manifest, exact ZIP inventory and SHA-256 verification.
- Separate Windows updater helper, journalled replacement and rollback; device `data` preserved.
- Active sessions or MCP processes block installation instead of being terminated.

This is the one-time updater bootstrap. Future stable releases can be installed with
the Update button. Stable checks intentionally exclude preview releases.

Download the **windows-x64.zip** to run the app. The **source.zip** contains complete
corresponding source with modified submodules; source/build instructions are included.
Never add your `data` folder to a distributable archive or copy identity between PCs.

Preview limitations: no clean Windows VM or complete two-PC acceptance yet;
Sciter native UI is not verified by an ordinary browser. See docs/agent/STATUS.md
for evidence and current limitations. No service, driver, autostart or firewall changes.
