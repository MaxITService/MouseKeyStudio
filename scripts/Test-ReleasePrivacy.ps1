param(
    [string[]]$BinaryFiles = @(
        "$PSScriptRoot/../target/release/mousekey-studio.exe",
        "$PSScriptRoot/../target/release/mks-agent.exe",
        "$PSScriptRoot/../target/release/mks-overlay.exe",
        "$PSScriptRoot/../target/release/mks.exe"
    ),
    [string]$SourceDirectory = (Split-Path -Parent $PSScriptRoot)
)
$ErrorActionPreference = 'Stop'
if (!$BinaryFiles.Count) { throw 'No release binaries supplied.' }
$forbidden = @($env:USERPROFILE, [IO.Path]::GetFullPath($SourceDirectory)) |
    Where-Object { $_ } | ForEach-Object { $_; $_.Replace('\', '/') } | Select-Object -Unique
foreach ($file in $BinaryFiles) {
    $bytes = [IO.File]::ReadAllBytes([IO.Path]::GetFullPath($file))
    # Hex searches also detect unaligned UTF-16 literals without decoding the PE.
    $hex = [Convert]::ToHexString($bytes)
    foreach ($path in $forbidden) {
        foreach ($encoding in @([Text.Encoding]::UTF8, [Text.Encoding]::Unicode)) {
            if ($hex.Contains([Convert]::ToHexString($encoding.GetBytes($path)))) {
                throw "Builder path embedded in $file. Rebuild with Build-Compact.ps1."
            }
        }
    }
}
Write-Output "PASS: $($BinaryFiles.Count) release binaries contain no builder home/checkout paths (UTF-8/UTF-16)."
