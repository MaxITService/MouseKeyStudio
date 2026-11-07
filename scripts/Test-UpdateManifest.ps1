param([Parameter(Mandatory)][string]$MsiFile)
# Packaging integration only: verifier stand-ins test the handoff and failure
# boundary. They do not claim cryptographic validity and never install the MSI.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/Read-MsiTable.ps1"
$properties = @{}
Read-MsiTable $MsiFile 'SELECT `Property`, `Value` FROM `Property`' @('Name', 'Value') |
    ForEach-Object { $properties[$_.Name] = $_.Value }
$scratch = Join-Path ([IO.Path]::GetTempPath()) ('mks-manifest-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scratch | Out-Null
$signature = Join-Path $scratch 'test-only.minisig'
$output = Join-Path $scratch 'latest.json'
$reject = Join-Path $scratch 'reject.cmd'
$accept = Join-Path $scratch 'accept.cmd'
Set-Content -LiteralPath $signature -Value 'TEST FIXTURE ONLY - not a real signature' -Encoding ascii
Set-Content -LiteralPath $reject -Value '@exit /b 7' -Encoding ascii
Set-Content -LiteralPath $accept -Value '@exit /b 0' -Encoding ascii
$argsForManifest = @{
    Version = $properties.ProductVersion
    MsiFile = [IO.Path]::GetFullPath($MsiFile)
    SignatureFile = $signature
    PublicKey = 'test-only-public-key'
    OutputFile = $output
}
function Expect-Rejection([string]$ExpectedMessage, [scriptblock]$Action) {
    $errorText = $null
    try { & $Action | Out-Null } catch { $errorText = $_.Exception.Message }
    if (!$errorText -or $errorText -notlike "*$ExpectedMessage*") {
        throw "Expected rejection containing '$ExpectedMessage'; got '$errorText'."
    }
    if (Test-Path -LiteralPath $output) { throw 'Rejected input produced a manifest.' }
}
Expect-Rejection 'signature verification failed' {
    & "$PSScriptRoot/New-UpdateManifest.ps1" @argsForManifest -Minisign $reject
}
$wrongVersion = $argsForManifest.Clone(); $wrongVersion.Version = '999.0.0'
Expect-Rejection 'must match the actual MSI' {
    & "$PSScriptRoot/New-UpdateManifest.ps1" @wrongVersion -Minisign $accept
}
$wrongVersion.Version = '1.0.0+custom.1'
Expect-Rejection 'stable numeric' {
    & "$PSScriptRoot/New-UpdateManifest.ps1" @wrongVersion -Minisign $accept
}
& "$PSScriptRoot/New-UpdateManifest.ps1" @argsForManifest -Minisign $accept | Out-Null
$manifest = Get-Content -LiteralPath $output -Raw | ConvertFrom-Json
$asset = $manifest.assets[0]
if ($manifest.version -ne $properties.ProductVersion -or
    $manifest.app_id -ne 'io.github.maxitservice.mousekeystudio' -or
    $asset.url -ne "https://github.com/MaxITService/MouseKeyStudio/releases/download/v$($properties.ProductVersion)/MouseKeyStudio-windows-x64.msi" -or
    $asset.sha256 -ne (Get-FileHash -LiteralPath $MsiFile -Algorithm SHA256).Hash.ToLowerInvariant() -or
    $asset.signature.Trim() -ne 'TEST FIXTURE ONLY - not a real signature' -or
    $asset.size -ne (Get-Item -LiteralPath $MsiFile).Length -or
    $asset.os -ne 'windows' -or $asset.arch -ne 'x86_64' -or $asset.format -ne 'msi') {
    throw 'Manifest does not describe the exact MSI and independent release destination.'
}
Write-Output 'PASS: verifier failure writes nothing; version/product gate; exact MSI hash, size, signature and release URL. Test-only verifier; no installation.'
