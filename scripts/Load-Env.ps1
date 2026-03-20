<#
.SYNOPSIS
    Load .env into the current PowerShell session (service-scoped, not machine-global).
.EXAMPLE
    . .\scripts\Load-Env.ps1
#>
[CmdletBinding()]
param()

$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
$EnvFile = Join-Path $Root '.env'

if (-not (Test-Path $EnvFile)) {
    Write-Error ".env not found at $EnvFile`nCopy .env.example to .env and fill in your credentials:`n  Copy-Item .env.example .env"
    return
}

Get-Content $EnvFile | ForEach-Object {
    $line = $_.Trim()
    if ($line -eq '' -or $line.StartsWith('#')) { return }
    $parts = $line -split '=', 2
    if ($parts.Count -eq 2) {
        $name = $parts[0].Trim()
        $value = $parts[1].Trim().Trim('"').Trim("'")
        # Process-scoped only — does NOT modify the machine or user registry.
        [System.Environment]::SetEnvironmentVariable($name, $value, 'Process')
        Write-Verbose "  $name = $($value.Substring(0, [Math]::Min(20, $value.Length)))..."
    }
}

Write-Host "Loaded OpenTelemetry environment from $EnvFile"
