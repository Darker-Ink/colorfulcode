<#
.SYNOPSIS
    Backs up a folder and prunes old archives.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Source,
    [int]$Keep = 5,
    [switch]$WhatIf
)

$ErrorActionPreference = 'Stop'
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$target = Join-Path $env:TEMP "backup-$stamp.zip"

function Remove-OldBackups {
    param([string]$Path, [int]$Count)
    Get-ChildItem -Path $Path -Filter '*.zip' |
        Sort-Object LastWriteTime -Descending |
        Select-Object -Skip $Count |
        ForEach-Object { Remove-Item $_.FullName -Force }
}

if (-not (Test-Path $Source)) {
    throw "Source '$Source' does not exist"
}

$settings = @{
    Level = 'Optimal'
    Size  = 1.5MB
}

try {
    Compress-Archive -Path "$Source\*" -DestinationPath $target -CompressionLevel $settings.Level
    Write-Host "Saved to $target (`$($items.Count) items)" -ForegroundColor Green
} catch [System.IO.IOException] {
    Write-Warning "Backup failed: $($_.Exception.Message)"
    exit 1
}

$summary = @"
Backup: $target
Keep:   $Keep
"@
if ($Keep -gt 0 -and $settings.Size -le 2MB) { Remove-OldBackups -Path $env:TEMP -Count $Keep }
[Math]::Round($settings.Size / 1KB, 2)
