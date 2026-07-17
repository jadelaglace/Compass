# Vault Git Backup

> P5.8 provides a local, daily Git backup for the Vault. It records the result of edits made through Obsidian or Compass; it is not part of the Compass API, index, watcher, or scoring path.

## Boundary

- The Vault frontmatter and Markdown remain authoritative. Git stores an auditable history only; it never supplies query data or rolls back a Compass write.
- Backup runs as an external PowerShell script. The Rust binary does not spawn Git and Git failure cannot block HTTP requests, indexing, or Vault writes.
- The Vault must have its **own Git repository**. The script refuses to operate on the parent Compass repository, so the root project's ignored `vault/` directory cannot be committed accidentally.
- The script never calls `pull`, `push`, `reset`, `checkout`, or `clean`. Remote replication belongs to P5.9.

## Tracked Scope

Each backup stages only the following paths, including additions, edits, deletes, and renames:

| Included | Reason |
|---|---|
| `**/*.md` | Vault notes and Templater templates |
| `.gitignore` | Vault backup policy |
| `.obsidian/app.json`, `appearance.json`, `community-plugins.json`, `core-plugins.json`, `hotkeys.json` | Stable Obsidian settings needed to reproduce the workspace |

Volatile workspace state, plugins, caches, `.compass/` indexes, databases, credentials, and any other file are left unstaged. The script also refuses to run when the Vault Git index already has staged changes; inspect or commit those changes manually before running it.

## One-Time Setup

Run these commands from the Compass repository after reviewing that `vault` is the intended Vault path:

```powershell
.\scripts\Backup-CompassVault.ps1 -VaultPath .\vault -Initialize
git -C .\vault config user.name "Your Name"
git -C .\vault config user.email "you@example.com"
.\scripts\Backup-CompassVault.ps1 -VaultPath .\vault
```

`-Initialize` only creates the dedicated Git repository and exits. It never creates a first commit before an explicit local Git identity exists. The next command produces the initial, allowlisted backup commit.

An optional remote may be added and pushed manually after confirming the content policy. The automatic task does not contact a remote.

## Daily Task

The following creates or updates a Windows task that runs daily at 23:00 for the current interactive user:

```powershell
.\scripts\Install-CompassVaultBackupTask.ps1 -VaultPath .\vault -DailyAt 23:00
```

The task uses `StartWhenAvailable`, so a missed run is started when the user next has an interactive session. Choose another `HH:mm` time with `-DailyAt`; `-WhatIf` previews task registration without changing Windows Task Scheduler.

Before installing the task, perform the one-time setup and one successful manual backup. To inspect the installed task:

```powershell
Get-ScheduledTask -TaskName "Compass Vault Backup"
Get-ScheduledTaskInfo -TaskName "Compass Vault Backup"
```

## Audit and Recovery

For every non-empty run, the backup script prints the staged name/status diff, the commit hash, and the commit stat. The scheduled task appends the same output to `%LOCALAPPDATA%\Compass\vault-backup.log`. A clean Vault produces no empty commit.

```powershell
git -C .\vault log --oneline --decorate
git -C .\vault show --stat HEAD
git -C .\vault show HEAD -- Knowledge/example.md
```

Recovery is deliberately a manual Git decision. Review a historical diff first, then use normal Git commands in the Vault repository if a restore is required. The backup scripts never restore working files.

## Verification

The integration check creates an isolated temporary Vault and validates initialization, the allowlist, no-op behavior, preserved Markdown content, and refusal of pre-existing staged changes:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\Test-CompassVaultBackup.ps1
```
