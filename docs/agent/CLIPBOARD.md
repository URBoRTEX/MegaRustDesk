# Human clipboard in the portable viewer

Owner request, 2026-10-27: Ctrl+C/Ctrl+V for text and files in both directions in
MegaRustDesk itself. Delivery uses the existing signed updater on both PCs.

## Baseline and implementation

0.3.5 forces clipboard off in LoginConfigHandler and session permission predicates.
Physical Ctrl+V is consumed by a text-only KeyEvent::Seq path. File transfer already
exists upstream through Windows Cliprdr/OLE, RustDesk messages and the connection
manager. Reuse that path; do not add a protocol, service, dependency or UI primitive.

Restore ordinary Ctrl+C/Ctrl+V in a human-controlled connected viewer. A physical
viewer enter/key action activates clipboard permissions for the current Manual
epoch. Existing clipboard/file-transfer settings and receiver permissions still
apply. Text and file clipboard have separate permissions. Initial login advertises
clipboard disabled until the human activates the viewer.

Do not revoke a granted operation merely because the pointer/focus leaves the
viewer: remote -> local paste requires using local Explorer. Ownership transfer to
Codex, pause, stop, disconnect and exit revoke access. Reclaim needs a fresh human
action. No clipboard API or file API is added to MCP; its tools cannot activate the
human permission.

Revocation must reach native Windows clipboard reads, queued callbacks, I/O sends,
native writes and delayed text workers. Native suspension uses an access-token
callback, not context destruction: upstream uninit can wait indefinitely while OLE
objects still have references. A generation token fences queued callbacks and
native OLE objects/waits across a revoke/reclaim transition. Non-agent/CM contexts
retain the upstream default. Each Sciter viewer remains a separate process.

First paste ordering: physical Ctrl+V captures the real V-down and preserves later
key-up/mouse events. Text snapshot is serialized before the key; the agent receiver
applies text synchronously. For files, force-refresh the current CF_HDROP formats
and wait for the matching operation ID acknowledged after native OleSetClipboard.
A constant synthetic format marker carries the ID; the existing Cliprdr response
retains its low flag bits and carries the ID in the remaining bits. No protobuf
type is added. Both ends require 0.3.6. Older/missing/failed ACK cancels paste rather
than using an old remote clipboard. The queue is bounded to 256 events/5 seconds;
an ordered release discards it before Codex input can begin.

Existing saved clipboard choices are migrated once per peer and then preserved.
The connection-manager native context also revokes access before teardown so its
pending waits can abort; incoming/outgoing file packets require that peer's current
file permission. Windows clipboard is process-global; multi-peer CM behavior is
not equivalent to independently isolated clipboards and needs separate acceptance.

The existing explicit text-paste button remains a fallback. The physical shortcut
uses native clipboard and must not turn a copied file into a text path or duplicate
the V keystroke. Clipboard contents, paths and bytes are excluded from receipts.

## Acceptance and verification

- Local -> remote and remote -> local: Unicode text, newlines, one file, multiple
  files and a directory; verify bytes in a disposable test directory.
- Copy a remote file, leave the viewer, paste into local Explorer.
- Pre-existing local copy before viewer activation; repeated paste; clipboard busy;
  disconnected, view-only, receiver clipboard/file permission denied.
- Handoff to Codex while a callback/text write/file response is queued: no clipboard
  read/write/send after revocation; old generations stay invalid after reclaim.
- MCP inventory unchanged; no local clipboard/file tool. Existing key-release,
  manual ownership, file-transfer toolbar and updater behavior remain intact.
- Pure policy and real native guard tests are distinct from browser fixture and
  physical two-PC evidence. Build alone does not prove clipboard transfer.

UI baseline: docs/UI_CONTRACT.md, existing Sciter toolbar and sidebar. No geometry,
tokens, new controls or layout changes are planned. Browser regression uses the
existing viewer fixture at its existing viewports. Physical/native clipboard gates
must be reported separately.
