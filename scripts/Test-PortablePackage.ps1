# Run without compiling or launching binaries; verify the archive boundary.
$ErrorActionPreference = 'Stop'
$scratch = Join-Path ([IO.Path]::GetTempPath()) ('openlogi-plus-package-test-' + [Guid]::NewGuid().ToString('N'))
$release = Join-Path $scratch 'release'
$output = Join-Path $scratch 'output'
New-Item -ItemType Directory -Path $release -Force | Out-Null
foreach ($name in @('mousekey-studio.exe', 'mks-agent.exe', 'mks-overlay.exe', 'mks.exe')) {
    Set-Content -LiteralPath (Join-Path $release $name) -Value "test payload: $name" -Encoding ascii
}
Set-Content -LiteralPath (Join-Path $release 'THIRD-PARTY-NOTICES.txt') -Value 'test notices'
Set-Content -LiteralPath (Join-Path $release 'config.toml') -Value 'private profile sentinel'
Set-Content -LiteralPath (Join-Path $release 'credentials.json') -Value 'private credential sentinel'
& "$PSScriptRoot/Package-Portable.ps1" -ReleaseDirectory $release -OutputDirectory $output
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead((Join-Path $output 'MouseKeyStudio-windows-x64.zip'))
try {
    $entries = @($zip.Entries | Where-Object { $_.Name } | ForEach-Object { $_.FullName.Replace('\', '/') })
    $expected = @(
        'MouseKeyStudio.exe', 'mks-agent.exe', 'mks-overlay.exe', 'mks.exe',
        'README.md', 'MACRO-AGENT-GUIDE.md', 'NAME-AND-ORIGIN.md', 'THIRD-PARTY-NOTICES.txt', 'Start-MouseKeyStudio.cmd', 'mousekey-studio-portable.toml',
        'LICENSE-MIT', 'LICENSE-APACHE', 'LICENSE-hidpp', 'LICENSE-gpui', 'LICENSE-plus-artwork'
    ) | ForEach-Object { "Mouse & Key Studio/$_" }
    if (Compare-Object ($expected | Sort-Object) ($entries | Sort-Object)) {
        throw "Unexpected portable archive entries: $entries"
    }
    $stream = [IO.StreamReader]::new($zip.GetEntry('Mouse & Key Studio/MouseKeyStudio.exe').Open())
    try {
        if ($stream.ReadToEnd().Trim() -ne 'test payload: mousekey-studio.exe') {
            throw 'The GUI was not packaged under the launchable filename.'
        }
    } finally { $stream.Dispose() }
    $stream = [IO.StreamReader]::new($zip.GetEntry('Mouse & Key Studio/mousekey-studio-portable.toml').Open())
    try {
        if ($stream.ReadToEnd() -notmatch '(?m)^directory = "settings"\r?$') {
            throw 'Portable settings must default to the relative settings subfolder.'
        }
    } finally { $stream.Dispose() }
} finally { $zip.Dispose() }
$missing = Join-Path $scratch 'empty-release'
New-Item -ItemType Directory -Path $missing | Out-Null
$rejected = $false
try {
    & "$PSScriptRoot/Package-Portable.ps1" -ReleaseDirectory $missing -OutputDirectory (Join-Path $scratch 'missing-output')
} catch { $rejected = $true }
if (-not $rejected) { throw 'Missing binaries must prevent packaging.' }
Write-Output 'PASS: exact portable contents; private files excluded; GUI renamed; missing binaries rejected.'
Write-Output "Test artifacts: $scratch"
