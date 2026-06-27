param(
    [string]$Version = (Get-Content (Join-Path $PSScriptRoot "..\package.json") -Raw | ConvertFrom-Json).version
)

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$bundle = Join-Path $root "src-tauri\target\release\bundle"
$nsis = Join-Path $bundle "nsis\Token.Anxiety.Dashboard_${Version}_x64-setup.exe"
$msi = Join-Path $bundle "msi\Token.Anxiety.Dashboard_${Version}_x64_en-US.msi"

Write-Host "Release asset paths (aligned with GitHub naming):"
Write-Host "  NSIS: $nsis"
Write-Host "  MSI : $msi"
Write-Host ""
Write-Host "Upload example (new release tag only; do NOT use --clobber on existing releases):"
Write-Host "  gh release create v$Version `"$nsis`" `"$msi`" --title `"v$Version`""