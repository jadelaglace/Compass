# Vault Cross-Device Sync

> P5.9 synchronizes the Vault's already-authoritative files between devices. Compass does not transport files, choose a winner, or merge content. After a sync, each device rebuilds its local SQLite projection from the Vault.

## Scope and Boundary

- Use one synchronization method for a Vault at a time. Syncthing is the supported primary mode; WebDAV is an alternative only when its client preserves conflicts as separate files.
- Sync Markdown and the same stable Obsidian settings listed in [VAULT_BACKUP.md](VAULT_BACKUP.md). Do not sync `.git/`, `.compass/`, database files, logs, caches, plugins, or `workspace*.json`.
- Each device owns its own Vault Git repository and local Compass index. Git history and SQLite are derived local state, never synchronization input.
- Compass keeps its existing authority model: a synced Markdown/frontmatter file is treated exactly like an Obsidian edit. It does not call Syncthing, WebDAV, or a remote API.

## Syncthing (Primary)

1. Create a Syncthing folder for the Vault on each device and share it in `Send & Receive` mode.
2. Add ignore rules for `.git/`, `.compass/`, `.obsidian/workspace.json`, `.obsidian/workspace-mobile.json`, `.obsidian/plugins/`, and `.obsidian/cache/`. Keep the folder root and ignore rules identical on every device.
3. Enable a versioning policy in Syncthing before enabling more than one writer. Versioning is an additional recovery layer; P5.8 Git backup remains independent.
4. Complete an initial one-way seed and verify the receiving device has the expected Markdown files before opening both Vaults for editing.
5. Start or restart Compass on each device after the initial transfer. Startup rebuilds the local SQLite projection from the synchronized Vault.

Syncthing conflict copies use names such as `note.sync-conflict-20260712-120000-DEVICE.md`. Compass preserves these files but excludes them from watcher processing and all index rebuilds, so a duplicate `id` cannot enter the entity index.

## WebDAV (Alternative)

WebDAV is suitable only through a client with versioning or conflict-copy retention. Configure it to retain a concurrent edit as `*.webdav-conflict-<timestamp>.md`; this is the convention Compass recognizes and excludes from indexing.

Do not run two opposite one-way mirror jobs against an active Vault. A WebDAV client that silently applies last-write-wins is not a supported P5.9 configuration because it can discard an authoring result before it reaches Git history. Sync one device at a time until the client has demonstrated conflict-copy retention with the validation scenario below.

## Conflict Resolution

1. Pause the folder on the affected devices and stop editing the primary note.
2. Keep both the primary file and the conflict copy. Compare their frontmatter and body manually; do not let Compass or a sync job choose one.
3. Merge the intended content into the primary filename. Retain one stable `id` in that primary file.
4. Delete the resolved conflict copy only after the merged primary file is saved and verified.
5. Resume synchronization, wait until all devices agree, then restart Compass on each device. The startup rebuild replaces the local SQLite projection from the resolved Vault state.

Conflict files are deliberately not auto-merged, auto-deleted, or indexed. Their presence is the operator signal that manual resolution is required.

## Validation

The repository regression checks the two Compass-owned portions of the workflow: conflict-copy isolation and rebuild from the primary note.

```powershell
cd compass-core
cargo test rebuild_excludes_sync_conflict_copies_and_keeps_the_primary_note --release
cargo test sync_conflict_copies_are_identified_case_insensitively --release
cargo test sync_conflict_round_trip_rebuilds_from_the_resolved_primary_note --release
```

For a real two-device acceptance check, create a conflicting edit of the same Markdown note while both devices are offline, let the selected sync tool exchange files, and confirm all of the following:

1. The primary note and a separately named conflict copy exist.
2. Compass lists only the primary entity after a restart/rebuild.
3. The conflict copy is still available for manual comparison.
4. After manual resolution and conflict-file removal, restarting Compass produces a clean index whose entity count and search results match the primary Vault files.

The Syncthing path was verified with two isolated local Syncthing 2.1.2 instances on 2026-07-12: an offline concurrent edit produced `.sync-conflict-*.md` copies, and an isolated Compass startup rebuild resolved the entity to the primary `Knowledge/note.md` path rather than a conflict copy.
