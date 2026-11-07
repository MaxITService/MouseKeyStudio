param([string]$MsiFile = (Join-Path $PSScriptRoot '../target/portable/MouseKeyStudio-windows-x64.msi'))
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/Read-MsiTable.ps1"
$properties = @{}
Read-MsiTable $MsiFile 'SELECT `Property`, `Value` FROM `Property`' @('Name', 'Value') | ForEach-Object { $properties[$_.Name] = $_.Value }
if ($properties.ProductName -ne 'Mouse & Key Studio' -or $properties.Manufacturer -ne 'Maxim Fomin') { throw 'Incorrect MSI product identity.' }
if ($properties.UpgradeCode -ne '{A6C0A57B-5A03-4F52-AE30-7B6FB405C500}') { throw 'MSI upgrade identity is not the independent x64 identity.' }
if ($properties.ALLUSERS -eq '1') { throw 'MSI must be per-user.' }
$files = @(Read-MsiTable $MsiFile 'SELECT `FileName` FROM `File`' @('Name') | ForEach-Object { ($_.Name -split '\|')[-1] })
$expected = @('MouseKeyStudio.exe', 'mks-agent.exe', 'mks-overlay.exe', 'mks.exe',
    'README.md', 'MACRO-AGENT-GUIDE.md', 'NAME-AND-ORIGIN.md', 'THIRD-PARTY-NOTICES.txt',
    'LICENSE-MIT', 'LICENSE-APACHE', 'LICENSE-hidpp', 'LICENSE-gpui', 'LICENSE-plus-artwork')
if (Compare-Object ($files | Sort-Object) ($expected | Sort-Object)) { throw "Unexpected MSI files: $files" }
$registry = @(Read-MsiTable $MsiFile 'SELECT `Root`, `Key`, `Name`, `Value` FROM `Registry`' @('Root', 'Key', 'Name', 'Value'))
if (!($registry | Where-Object { $_.Root -eq '1' -and $_.Key -eq 'Software\MouseKeyStudio' -and $_.Name -eq 'InstallLocation' -and $_.Value -eq '[INSTALLFOLDER]' })) { throw 'Missing per-user, path-bound MSI receipt.' }
if ($registry | Where-Object { $_.Key -match 'OpenLogi' }) { throw 'MSI touches upstream registration.' }
$targets = @(Read-MsiTable $MsiFile 'SELECT `Target` FROM `Wix4CloseApplication`' @('Target') | ForEach-Object Target)
if (Compare-Object ($targets | Sort-Object) (@('MouseKeyStudio.exe', 'mks-agent.exe', 'mks-overlay.exe', 'mks.exe') | Sort-Object)) { throw "Incorrect process-close targets: $targets" }
Write-Output "PASS: MSI $($properties.ProductVersion), independent identity, exact files, per-user receipt, own processes only. Read-only; nothing installed."
