[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$VaultPath,

    [switch]$Initialize,

    [string]$AuditLogPath,

    [string]$GitExecutable = "git"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ConfigPaths = @(
    ".gitignore",
    ".obsidian/app.json",
    ".obsidian/appearance.json",
    ".obsidian/community-plugins.json",
    ".obsidian/core-plugins.json",
    ".obsidian/hotkeys.json"
)

function Write-Audit {
    param([Parameter(Mandatory = $true)][string]$Message)

    $line = "{0} {1}" -f (Get-Date -Format "yyyy-MM-ddTHH:mm:ssK"), $Message
    Write-Output $line

    if ($script:ResolvedAuditLogPath) {
        Add-Content -LiteralPath $script:ResolvedAuditLogPath -Value $line -Encoding utf8
    }
}

function Invoke-Git {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    $output = @(& $script:GitPath -C $script:Vault @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Arguments -join ' ') failed: $($output -join [Environment]::NewLine)"
    }
    return $output
}

function Test-AllowedBackupPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    $normalized = $Path.Replace('\', '/')
    if ($normalized -match "(?i)\.md$") {
        return $true
    }

    return $script:ConfigPaths -contains $normalized
}

function Get-ExistingOrTrackedPathspecs {
    $pathspecs = @(":(glob)**/*.md")
    foreach ($configPath in $script:ConfigPaths) {
        $exists = Test-Path -LiteralPath (Join-Path $script:Vault $configPath)
        $tracked = @(& $script:GitPath -C $script:Vault ls-files -- $configPath)
        if ($exists -or $tracked.Count -gt 0) {
            $pathspecs += $configPath
        }
    }
    return $pathspecs
}

try {
    $gitCommand = Get-Command $GitExecutable -CommandType Application -ErrorAction Stop
    $script:GitPath = $gitCommand.Source
    $script:Vault = (Resolve-Path -LiteralPath $VaultPath -ErrorAction Stop).Path
    if (-not (Test-Path -LiteralPath $script:Vault -PathType Container)) {
        throw "Vault path is not a directory: $VaultPath"
    }
    $script:ConfigPaths = $ConfigPaths

    if ($AuditLogPath) {
        $auditDirectory = Split-Path -Parent $AuditLogPath
        if ($auditDirectory) {
            New-Item -ItemType Directory -Force -Path $auditDirectory | Out-Null
        }
        $script:ResolvedAuditLogPath = [System.IO.Path]::GetFullPath($AuditLogPath)
    } else {
        $script:ResolvedAuditLogPath = $null
    }

    $isDedicatedRepository = $false
    if (Test-Path -LiteralPath (Join-Path $script:Vault ".git")) {
        $topLevel = @(Invoke-Git @("rev-parse", "--show-toplevel"))
        $trimChars = [char[]]@([char]'\', [char]'/')
        $isDedicatedRepository = $topLevel.Count -eq 1 -and
            [string]::Equals(
                [System.IO.Path]::GetFullPath($topLevel[0]).TrimEnd($trimChars),
                [System.IO.Path]::GetFullPath($script:Vault).TrimEnd($trimChars),
                [System.StringComparison]::OrdinalIgnoreCase
            )
    }

    if (-not $isDedicatedRepository) {
        if (-not $Initialize) {
            throw "Vault is not its own Git repository. Run again with -Initialize after reviewing the Vault path."
        }
        & $script:GitPath -C $script:Vault init | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "git init failed for Vault: $script:Vault"
        }
        Write-Audit "Initialized dedicated Vault Git repository. Configure its user.name and user.email, then run backup again."
        return
    }

    $preStaged = @(Invoke-Git @("diff", "--cached", "--name-only"))
    if ($preStaged.Count -gt 0) {
        throw "Refusing to commit because the Vault index already contains staged changes: $($preStaged -join ', ')"
    }

    $pathspecs = @(Get-ExistingOrTrackedPathspecs)
    Invoke-Git (@("add", "-A", "--") + $pathspecs) | Out-Null

    $stagedNameStatus = @(Invoke-Git @("diff", "--cached", "--name-status"))
    if ($stagedNameStatus.Count -eq 0) {
        Write-Audit "No eligible Vault changes to back up."
        return
    }

    $stagedFiles = @(Invoke-Git @("diff", "--cached", "--name-only"))
    foreach ($stagedFile in $stagedFiles) {
        if (-not (Test-AllowedBackupPath $stagedFile)) {
            Invoke-Git (@("restore", "--staged", "--") + $stagedFiles) | Out-Null
            throw "Backup allowlist violation: $stagedFile"
        }
    }

    Write-Audit "Vault diff to commit:"
    foreach ($line in $stagedNameStatus) {
        Write-Audit "  $line"
    }

    $message = "backup(vault): $(Get-Date -Format 'yyyy-MM-dd')"
    try {
        Invoke-Git @("commit", "-m", $message) | ForEach-Object { Write-Audit $_ }
    } catch {
        Invoke-Git (@("restore", "--staged", "--") + $stagedFiles) | Out-Null
        throw
    }

    $commit = (@(Invoke-Git @("rev-parse", "HEAD")))[0]
    Write-Audit "Created Vault backup commit $commit."
    Invoke-Git @("show", "--stat", "--oneline", "--summary", "HEAD") |
        ForEach-Object { Write-Audit $_ }
} catch {
    if ($script:ResolvedAuditLogPath) {
        Write-Audit "FAILED: $($_.Exception.Message)"
    }
    throw
}
