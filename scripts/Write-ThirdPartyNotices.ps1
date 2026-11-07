param([string]$OutputFile = (Join-Path $PSScriptRoot '../target/release/THIRD-PARTY-NOTICES.txt'))
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    $raw = & cargo metadata --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed.' }
    $metadata = ($raw | Out-String) | ConvertFrom-Json -AsHashtable -Depth 100
    $nodes = @{}
    foreach ($node in $metadata.resolve.nodes) { $nodes[$node.id] = $node }
    $packages = @{}
    foreach ($package in $metadata.packages) { $packages[$package.id] = $package }
    $seen = [Collections.Generic.HashSet[string]]::new()
    $pending = [Collections.Generic.Stack[string]]::new()
    foreach ($package in $metadata.packages | Where-Object { $_.name -in @('openlogi-desktop', 'openlogi-agent', 'openlogi-overlay', 'openlogi') }) {
        $pending.Push($package.id)
    }
    while ($pending.Count) {
        $id = $pending.Pop()
        if (!$seen.Add($id)) { continue }
        foreach ($dependency in $nodes[$id].deps) {
            if (@($dependency.dep_kinds | Where-Object { $_.kind -ne 'dev' }).Count) { $pending.Push($dependency.pkg) }
        }
    }
    $text = [Text.StringBuilder]::new()
    [void]$text.AppendLine('Mouse & Key Studio — third-party source notices')
    [void]$text.AppendLine('Original OpenLogi: https://github.com/AprilNEA/OpenLogi (MIT OR Apache-2.0).')
    [void]$text.AppendLine('Dependencies reachable from the Windows release binaries, excluding dev-only edges.')
    [void]$text.AppendLine('Cargo feature unification can include additional notices. No local source paths are recorded.')
    foreach ($id in $seen | Sort-Object) {
        $package = $packages[$id]
        if ($id -in $metadata.workspace_members) { continue }
        [void]$text.AppendLine("`n============================================================")
        [void]$text.AppendLine("$($package.name) $($package.version)")
        [void]$text.AppendLine("License expression: $($package.license)")
        [void]$text.AppendLine("Authors: $($package.authors -join ', ')")
        [void]$text.AppendLine("Repository: $($package.repository)")
        $directory = Split-Path -Parent $package.manifest_path
        $files = @(Get-ChildItem -LiteralPath $directory -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)([._-]|$)' })
        if ($package.license_file) { $files += Get-Item -LiteralPath (Join-Path $directory $package.license_file) }
        foreach ($folder in @('licenses', 'LICENSES')) {
            $path = Join-Path $directory $folder
            if (Test-Path -LiteralPath $path -PathType Container) { $files += Get-ChildItem -LiteralPath $path -File }
        }
        foreach ($file in $files | Sort-Object FullName -Unique) {
            if ($file.Length -gt 4MB) { throw "Unexpectedly large license file for $($package.name)." }
            [void]$text.AppendLine("`n--- $($file.Name) ---")
            [void]$text.AppendLine([IO.File]::ReadAllText($file.FullName))
        }
    }
    [void]$text.AppendLine("`n============================================================")
    [void]$text.AppendLine('WiX Toolset 6.0.2 installer runtime components (unmodified).')
    [void]$text.AppendLine('Copyright (c) .NET Foundation and contributors.')
    [void]$text.AppendLine('Complete corresponding source: https://github.com/wixtoolset/wix/tree/v6.0.2')
    [void]$text.AppendLine('Source archive: https://github.com/wixtoolset/wix/archive/refs/tags/v6.0.2.tar.gz')
    [void]$text.AppendLine([IO.File]::ReadAllText((Join-Path $PSScriptRoot '../packaging/windows/LICENSE-WiX.txt')))
    $output = [IO.Path]::GetFullPath($OutputFile)
    [IO.Directory]::CreateDirectory((Split-Path -Parent $output)) | Out-Null
    [IO.File]::WriteAllText($output, $text.ToString(), [Text.UTF8Encoding]::new($false))
    Write-Output "Third-party notices: $output"
} finally { Pop-Location }
