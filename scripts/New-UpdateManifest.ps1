param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$MsiFile,
    [Parameter(Mandatory)][string]$SignatureFile,
    [Parameter(Mandatory)][string]$PublicKey,
    [string]$Minisign = 'minisign',
    [string]$OutputFile = (Join-Path $PSScriptRoot '../target/portable/latest.json')
)
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Use a stable numeric x.y.z version; build metadata is not an upgrade.' }
if ((Split-Path -Leaf $MsiFile) -ne 'MouseKeyStudio-windows-x64.msi') { throw 'Expected the x64 Mouse & Key Studio MSI.' }
foreach ($file in @($MsiFile, $SignatureFile)) {
    if (!(Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing update input: $file" }
}
. "$PSScriptRoot/Read-MsiTable.ps1"
$properties = @{}
Read-MsiTable $MsiFile 'SELECT `Property`, `Value` FROM `Property`' @('Name', 'Value') | ForEach-Object { $properties[$_.Name] = $_.Value }
if ($properties.ProductName -ne 'Mouse & Key Studio' -or $properties.ProductVersion -ne $Version) {
    throw 'The manifest version and product must match the actual MSI database.'
}
& $Minisign -V -m $MsiFile -x $SignatureFile -P $PublicKey.Trim()
if ($LASTEXITCODE -ne 0) { throw 'MSI signature verification failed; no manifest written.' }
$url = "https://github.com/MaxITService/MouseKeyStudio/releases/download/v$Version"
$manifest = [ordered]@{
    schema_version = 1
    app_id = 'io.github.maxitservice.mousekeystudio'
    version = $Version
    assets = @([ordered]@{
        name = 'MouseKeyStudio-windows-x64.msi'
        url = "$url/MouseKeyStudio-windows-x64.msi"
        sha256 = (Get-FileHash -LiteralPath $MsiFile -Algorithm SHA256).Hash.ToLowerInvariant()
        signature = [IO.File]::ReadAllText([IO.Path]::GetFullPath($SignatureFile))
        size = (Get-Item -LiteralPath $MsiFile).Length
        os = 'windows'; arch = 'x86_64'; format = 'msi'
    })
}
$output = [IO.Path]::GetFullPath($OutputFile)
[IO.Directory]::CreateDirectory((Split-Path -Parent $output)) | Out-Null
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $output -Encoding utf8NoBOM
Write-Output "Verified update manifest: $output"
