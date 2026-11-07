# Manual Windows packages

The public distribution is **Mouse & Key Studio**, repository
`MaxITService/MouseKeyStudio`. Only Windows x64 packages are automated here.
macOS/Linux code is retained from upstream; those release workflows are inactive.

## GitHub Actions

Open **Actions → Mouse & Key Studio Windows packages → Run workflow** on `main`.
The workflow is `workflow_dispatch` only, has read-only repository permissions,
never runs on PRs or pushes, and neither publishes a release nor uses signing
secrets. Download its artifacts within 90 days:

- `MouseKeyStudio-windows-x64.zip` — portable, settings in its `settings` subfolder.
- `MouseKeyStudio-windows-x64.msi` — current-user installation, separate settings.

The optional input is a **public** minisign verification key, not a private key.
Without it the application explicitly disables automatic updates. Executables
and MSI from this workflow are not Authenticode signed, so Windows may show an
unknown-publisher warning. The workflow tests storage, updater policy, catalog
parity and archive contents before producing its artifacts.

## Local compact build

Use the pinned Rust toolchain, the MSVC build tools and Windows SDK. No device
driver installation is required for ordinary Windows HID access. Run from the
repository root in PowerShell:

```powershell
$env:RUST_MIN_STACK = '33554432'
$env:CARGO_INCREMENTAL = '0'
$env:CARGO_PROFILE_RELEASE_OPT_LEVEL = 'z'
$env:CARGO_PROFILE_RELEASE_LTO = 'thin'
$env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = '1'
$env:CARGO_PROFILE_RELEASE_STRIP = 'symbols'
cargo build --locked --release -p openlogi-desktop -p openlogi-agent -p openlogi-overlay -p openlogi --bin mousekey-studio --bin mks-agent --bin mks-overlay --bin mks
./scripts/Write-ThirdPartyNotices.ps1
./scripts/Package-Portable.ps1
```

The Rust package names retain their source identity; shipped executables use
this distribution's names. MSI packaging requires .NET SDK 6+ and WiX 6.0.2
with matching UI and Util extensions. The workflow shows the pinned setup.
`./scripts/Package-Msi.ps1 -Wix <path-to-wix.exe>` builds without installing,
launching or changing autostart. Its validation script opens the MSI database
read-only; runtime upgrade/uninstall testing needs a disposable Windows session.

## Publishing updates manually

1. Keep private signing keys outside the repository and build artifacts. Create
   a minisign key pair using the official minisign tooling and back up the private
   key securely. Build the app with the matching base64 public-key line in
   `MKS_UPDATE_MINISIGN_PUBLIC_KEY` (or the manual workflow input).
2. Increment the actual numeric package version for every public update. Semver
   build metadata (`+custom.N`) alone does not make an update newer, and Windows
   MSI compares the first three version components. Build matching ZIP and MSI.
3. Sign the MSI using minisign and verify it using your public key. Generate
   `latest.json` using `scripts/New-UpdateManifest.ps1` with that MSI and its
   `.minisig`. The generator verifies the signature and records SHA256 and size.
4. Create a new release in **this** repository and upload the ZIP, MSI,
   `.minisig`, and `latest.json`. The application reads
   `https://github.com/MaxITService/MouseKeyStudio/releases/latest/download/latest.json`.
   Do not overwrite an existing published release with different bytes.
5. Test a signed upgrade in a disposable session before recommending it. Portable
   copies continue to update manually and never install an MSI automatically.

There is no publishing job, upstream update-host fallback, or automatic PR build.
Keep original notices and [origin documentation](NAME-AND-ORIGIN.md) in every
source and binary distribution. Read [portable storage recovery](PORTABLE.md)
before changing a settings folder.
