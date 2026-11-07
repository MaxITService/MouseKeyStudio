param(
    [string]$ProjectDirectory = (Split-Path -Parent $PSScriptRoot),
    [string]$TargetDirectory = (Join-Path $PSScriptRoot '../target')
)
$ErrorActionPreference = 'Stop'
$project = (Resolve-Path -LiteralPath $ProjectDirectory).Path
$target = [IO.Path]::GetFullPath($TargetDirectory)
$values = @{
    RUST_MIN_STACK = '33554432'
    CARGO_INCREMENTAL = '0'
    CARGO_PROFILE_RELEASE_OPT_LEVEL = 'z'
    CARGO_PROFILE_RELEASE_LTO = 'thin'
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS = '1'
    CARGO_PROFILE_RELEASE_STRIP = 'symbols'
    CARGO_TARGET_DIR = $target
}
# Panic locations must not disclose the builder's username or checkout path.
# Encoded flags preserve paths with spaces. Later, more specific mappings win.
$flags = [Collections.Generic.List[string]]::new()
foreach ($mapping in @(
    @($env:USERPROFILE, '/build/user'),
    @($target, '/build/target'),
    @($project, '/src/mousekey-studio')
)) {
    if ($mapping[0]) {
        foreach ($path in @($mapping[0], $mapping[0].Replace('\', '/')) | Select-Object -Unique) {
            $flags.Add("--remap-path-prefix=$path=$($mapping[1])")
        }
    }
}
$values.CARGO_ENCODED_RUSTFLAGS = $flags -join [char]31
$previous = @{}
foreach ($name in $values.Keys) {
    $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
    [Environment]::SetEnvironmentVariable($name, $values[$name], 'Process')
}
try {
    Push-Location -LiteralPath $project
    try {
        & cargo build --locked --release -p openlogi-desktop -p openlogi-agent -p openlogi-overlay -p openlogi `
            --bin mousekey-studio --bin mks-agent --bin mks-overlay --bin mks
        if ($LASTEXITCODE) { throw "Compact build failed ($LASTEXITCODE)." }
    } finally { Pop-Location }
} finally {
    foreach ($name in $previous.Keys) {
        [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process')
    }
}
