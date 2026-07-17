[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$VaultPath,

    [ValidatePattern('^(?:[01]\d|2[0-3]):[0-5]\d$')]
    [string]$DailyAt = "23:00",

    [ValidateNotNullOrEmpty()]
    [string]$TaskName = "Compass Vault Backup"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$vault = (Resolve-Path -LiteralPath $VaultPath -ErrorAction Stop).Path
$backupScript = Join-Path $PSScriptRoot "Backup-CompassVault.ps1"
if (-not (Test-Path -LiteralPath $backupScript -PathType Leaf)) {
    throw "Backup script not found: $backupScript"
}

$powershell = (Get-Command powershell.exe -CommandType Application -ErrorAction Stop).Source
$auditLog = Join-Path $env:LOCALAPPDATA 'Compass\vault-backup.log'
$arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{0}" -VaultPath "{1}" -AuditLogPath "{2}"' -f $backupScript, $vault, $auditLog

$action = New-ScheduledTaskAction -Execute $powershell -Argument $arguments
$trigger = New-ScheduledTaskTrigger -Daily -At $DailyAt
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable
$principal = New-ScheduledTaskPrincipal -UserId ([System.Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited

if ($PSCmdlet.ShouldProcess($TaskName, "register daily Vault backup task at $DailyAt")) {
    Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Description "Commits the Compass Vault allowlist to its dedicated local Git repository." -Force | Out-Null
    Write-Output "Registered '$TaskName' for $DailyAt. Audit log: $auditLog"
}
