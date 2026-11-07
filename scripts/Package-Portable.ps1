param(
    [string]$ReleaseDirectory = (Join-Path $PSScriptRoot '../target/release'),
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '../target/portable')
)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
$stage = Join-Path $OutputDirectory ('stage-' + [Guid]::NewGuid().ToString('N'))
$bundle = Join-Path $stage 'Mouse & Key Studio'
New-Item -ItemType Directory -Path $bundle -Force | Out-Null
# An explicit allowlist keeps profiles, local backups and logs out of the ZIP.
$binaries = @{
    'mousekey-studio.exe' = 'MouseKeyStudio.exe'
    'mks-agent.exe' = 'mks-agent.exe'
    'mks-overlay.exe' = 'mks-overlay.exe'
    'mks.exe' = 'mks.exe'
}
foreach ($entry in $binaries.GetEnumerator()) {
    Copy-Item -LiteralPath (Join-Path $ReleaseDirectory $entry.Key) -Destination (Join-Path $bundle $entry.Value)
}
Copy-Item -LiteralPath (Join-Path $ReleaseDirectory 'THIRD-PARTY-NOTICES.txt') -Destination $bundle
Copy-Item -LiteralPath (Join-Path $project 'docs/NAME-AND-ORIGIN.md') -Destination $bundle
foreach ($file in @('LICENSE-MIT', 'LICENSE-APACHE')) {
    Copy-Item -LiteralPath (Join-Path $project $file) -Destination $bundle
}
Copy-Item -LiteralPath (Join-Path $project 'docs/MACRO-AGENT-GUIDE.md') -Destination $bundle
Copy-Item -LiteralPath (Join-Path $project 'docs/PORTABLE.md') -Destination (Join-Path $bundle 'README.md')
Copy-Item -LiteralPath (Join-Path $project 'crates/openlogi-hidpp/LICENSE') -Destination (Join-Path $bundle 'LICENSE-hidpp')
Copy-Item -LiteralPath (Join-Path $project 'vendor/gpui-pre/LICENSE-APACHE') -Destination (Join-Path $bundle 'LICENSE-gpui')
Copy-Item -LiteralPath (Join-Path $project 'assets/plus/LICENSE') -Destination (Join-Path $bundle 'LICENSE-plus-artwork')
@'
# Keep this file beside the GUI, agent and overlay. Relative paths travel
# with the portable folder. The app can move the pointer to your user profile.
directory = "settings"
'@ | Set-Content -LiteralPath (Join-Path $bundle 'mousekey-studio-portable.toml') -Encoding utf8
@'
@echo off
start "" "%~dp0MouseKeyStudio.exe"
'@ | Set-Content -LiteralPath (Join-Path $bundle 'Start-MouseKeyStudio.cmd') -Encoding ascii
$archive = Join-Path $OutputDirectory 'MouseKeyStudio-windows-x64.zip'
Compress-Archive -LiteralPath $bundle -DestinationPath $archive -CompressionLevel Optimal -Force
Write-Output "Portable bundle: $archive"
