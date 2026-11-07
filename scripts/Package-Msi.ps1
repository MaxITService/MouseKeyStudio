param(
    [string]$ReleaseDirectory = (Join-Path $PSScriptRoot '../target/release'),
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '../target/portable'),
    [string]$Wix = 'wix'
)
$ErrorActionPreference = 'Stop'
$project = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$release = [IO.Path]::GetFullPath($ReleaseDirectory)
$gui = Join-Path $release 'mousekey-studio.exe'
foreach ($name in @('mousekey-studio.exe', 'mks-agent.exe', 'mks-overlay.exe', 'mks.exe', 'THIRD-PARTY-NOTICES.txt')) {
    if (!(Test-Path -LiteralPath (Join-Path $release $name) -PathType Leaf)) { throw "Missing MSI input: $name" }
}
$versionText = (Get-Item -LiteralPath $gui).VersionInfo.ProductVersion
if ($versionText -notmatch '^(\d+)\.(\d+)\.(\d+)') { throw 'GUI has no numeric product version.' }
$version = "$($Matches[1]).$($Matches[2]).$($Matches[3])"
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$msi = Join-Path ([IO.Path]::GetFullPath($OutputDirectory)) 'MouseKeyStudio-windows-x64.msi'
& $Wix build (Join-Path $project 'packaging/windows/MouseKeyStudio.wxs') -arch x64 `
    -ext WixToolset.UI.wixext/6.0.2 -ext WixToolset.Util.wixext/6.0.2 `
    '-d' "Version=$version" '-d' "ExeFile=$gui" `
    '-d' "AgentExeFile=$(Join-Path $release 'mks-agent.exe')" `
    '-d' "OverlayExeFile=$(Join-Path $release 'mks-overlay.exe')" `
    '-d' "CliExeFile=$(Join-Path $release 'mks.exe')" `
    '-d' "IconFile=$(Join-Path $project 'assets/plus/icon.ico')" `
    '-d' "NoticesFile=$(Join-Path $release 'THIRD-PARTY-NOTICES.txt')" `
    '-d' "ProjectRoot=$project/" -o $msi
if ($LASTEXITCODE -ne 0) { throw "WiX build failed ($LASTEXITCODE)." }
Write-Output "MSI package (not installed): $msi"
