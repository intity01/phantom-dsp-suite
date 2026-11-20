<#
.SYNOPSIS
    Builds the Windows plugins and the launcher, then lays them out under dist/windows/.
.DESCRIPTION
    Produces the layout Phantom Launcher looks for:
      dist/windows/vst3/<Name>.vst3/Contents/x86_64-win/<Name>.vst3
      dist/windows/clap/<Name>.clap
      dist/windows/phantom-launcher.exe
    Use -SkipBuild to repackage existing target/release outputs without rebuilding, and
    -Only to package a subset (e.g. -Only phantom-vox) without touching the other plugins.
#>
param([switch]$SkipBuild, [string[]]$Only)

$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$release = Join-Path $root "target\release"
$dist = Join-Path $root "dist\windows"

# Cargo package name -> display name used for the bundle and file names.
$plugins = [ordered]@{
    "phantom-eq"  = "Phantom EQ"
    "phantom-vox" = "Phantom Vox"
}
if ($Only) {
    $unknown = $Only | Where-Object { -not $plugins.Contains($_) }
    if ($unknown) { throw "Unknown plugin(s): $($unknown -join ', ')" }
    foreach ($k in @($plugins.Keys)) { if ($Only -notcontains $k) { $plugins.Remove($k) } }
}

if (-not $SkipBuild) {
    $pkgArgs = @($plugins.Keys | ForEach-Object { "-p", $_ }) + @("-p", "phantom-launcher")
    # Cargo reports progress on stderr, which Windows PowerShell turns into errors under
    # "Stop". Judge the build by its exit code instead.
    $ErrorActionPreference = "Continue"
    & cargo build --release @pkgArgs 2>&1 | ForEach-Object { "$_" }
    $ErrorActionPreference = "Stop"
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

foreach ($pkg in $plugins.Keys) {
    $name = $plugins[$pkg]
    $dll = Join-Path $release ($pkg.Replace("-", "_") + ".dll")
    if (-not (Test-Path $dll)) { throw "Missing build output: $dll" }

    $vst3Dir = Join-Path $dist "vst3\$name.vst3\Contents\x86_64-win"
    $clapDir = Join-Path $dist "clap"
    New-Item -ItemType Directory -Force -Path $vst3Dir, $clapDir | Out-Null
    Copy-Item $dll (Join-Path $vst3Dir "$name.vst3") -Force
    Copy-Item $dll (Join-Path $clapDir "$name.clap") -Force
    Write-Host "Packaged $name"
}

Copy-Item (Join-Path $release "phantom-launcher.exe") (Join-Path $dist "phantom-launcher.exe") -Force
Write-Host "Done: $dist"
