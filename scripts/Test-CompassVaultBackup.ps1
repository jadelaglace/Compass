[CmdletBinding()]
param(
    [string]$BackupScript
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not $BackupScript) {
    $BackupScript = Join-Path $PSScriptRoot "Backup-CompassVault.ps1"
}
if (-not (Test-Path -LiteralPath $BackupScript -PathType Leaf)) {
    throw "Backup script not found: $BackupScript"
}

function Assert-True {
    param([Parameter(Mandatory = $true)][bool]$Condition, [Parameter(Mandatory = $true)][string]$Message)
    if (-not $Condition) {
        throw "Assertion failed: $Message"
    }
}

function Invoke-TestGit {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)
    $output = @(& git @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Arguments -join ' ') failed: $($output -join [Environment]::NewLine)"
    }
    return $output
}

$root = Join-Path ([System.IO.Path]::GetTempPath()) ("compass-vault-backup-" + [Guid]::NewGuid())
$vault = Join-Path $root "vault"
$auditLog = Join-Path $root "audit\\vault-backup.log"

try {
    New-Item -ItemType Directory -Force -Path (Join-Path $vault ".obsidian") | Out-Null
    Set-Content -LiteralPath (Join-Path $vault "note.md") -Value "# Original" -Encoding utf8
    Set-Content -LiteralPath (Join-Path $vault ".obsidian\\app.json") -Value '{"theme":"dark"}' -Encoding utf8
    Set-Content -LiteralPath (Join-Path $vault ".obsidian\\workspace.json") -Value '{"volatile":true}' -Encoding utf8
    Set-Content -LiteralPath (Join-Path $vault "private.txt") -Value "do not back up" -Encoding utf8

    & $BackupScript -VaultPath $vault -Initialize
    Assert-True (Test-Path -LiteralPath (Join-Path $vault ".git")) "initialization should create a dedicated Git repository"
    Invoke-TestGit -C $vault config user.name "Compass Backup Test" | Out-Null
    Invoke-TestGit -C $vault config user.email "backup-test@example.invalid" | Out-Null
    $initialBackupOutput = @(& $BackupScript -VaultPath $vault -AuditLogPath $auditLog)
    Assert-True (($initialBackupOutput -join "`n") -match "Created Vault backup commit [0-9a-f]{40}\.") "backup audit should include the full commit hash"
    Assert-True (Test-Path -LiteralPath $auditLog -PathType Leaf) "scheduled backup audit log should be created outside the Vault"
    Assert-True ((Get-Content -Raw -LiteralPath $auditLog) -match "Created Vault backup commit [0-9a-f]{40}\.") "audit log should retain the full commit hash"
    Assert-True ((@(Invoke-TestGit -C $vault rev-list --count HEAD))[0] -eq "1") "initial eligible files should create one commit"
    Assert-True ((Get-Content -Raw -LiteralPath (Join-Path $vault "note.md")) -match "# Original") "backup must not rewrite Markdown"
    $initialNames = Invoke-TestGit -C $vault show --format= --name-only HEAD
    Assert-True ($initialNames -contains "note.md") "Markdown should be committed"
    Assert-True ($initialNames -contains ".obsidian/app.json") "allowlisted Obsidian config should be committed"
    Assert-True (-not ($initialNames -contains ".obsidian/workspace.json")) "volatile Obsidian state must stay uncommitted"
    Assert-True (-not ($initialNames -contains "private.txt")) "non-allowlisted files must stay uncommitted"

    $noChangeOutput = @(& $BackupScript -VaultPath $vault)
    Assert-True ($LASTEXITCODE -eq 0) "empty backup should succeed"
    Assert-True (($noChangeOutput -join "`n") -match "No eligible Vault changes") "empty backup should report no eligible changes"
    Assert-True ((@(Invoke-TestGit -C $vault rev-list --count HEAD))[0] -eq "1") "empty backup must not create an empty commit"

    Set-Content -LiteralPath (Join-Path $vault "note.md") -Value "# Updated" -Encoding utf8
    & $BackupScript -VaultPath $vault
    Assert-True ((@(Invoke-TestGit -C $vault rev-list --count HEAD))[0] -eq "2") "Markdown change should create a second commit"
    Assert-True ((Get-Content -Raw -LiteralPath (Join-Path $vault "note.md")) -match "# Updated") "committing must preserve updated Markdown"

    Set-Content -LiteralPath (Join-Path $vault "note.md") -Value "# Manually staged" -Encoding utf8
    Invoke-TestGit -C $vault add -- note.md | Out-Null
    $headBeforeRefusal = (@(Invoke-TestGit -C $vault rev-parse HEAD))[0]
    $refused = $false
    try {
        & $BackupScript -VaultPath $vault
    } catch {
        $refused = $_.Exception.Message -match "already contains staged changes"
    }
    Assert-True $refused "backup must refuse pre-existing staged changes"
    Assert-True ((@(Invoke-TestGit -C $vault rev-parse HEAD))[0] -eq $headBeforeRefusal) "refusal must not create a commit"

    Write-Output "Compass Vault backup integration checks passed."
} finally {
    if (Test-Path -LiteralPath $root) {
        Remove-Item -LiteralPath $root -Recurse -Force
    }
}
