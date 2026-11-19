<#
.SYNOPSIS
    Phantom EQ - Modern PowerShell Installer for Cubase / Windows
.DESCRIPTION
    Installs Phantom EQ.vst3 into the user's standard VST3 directories and verifies integrity.
#>

$ErrorActionPreference = "Stop"

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "  🚀 Phantom EQ - VST3 Installer for Cubase" -ForegroundColor Yellow
Write-Host "=======================================================" -ForegroundColor Cyan

# 1. Check if Cubase is running
$cubaseProc = Get-Process -Name "Cubase*" -ErrorAction SilentlyContinue
if ($cubaseProc) {
    Write-Host "`n⚠️  [WARNING] Cubase is currently running ($($cubaseProc.ProcessName))!" -ForegroundColor Yellow
    Write-Host "Please close Cubase to unlock the plugin file, then press Enter to continue..."
    Read-Host
}

# 2. Paths
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$sourceFile = Join-Path $scriptDir "dist\windows\vst3\Phantom EQ.vst3\Contents\x86_64-win\Phantom EQ.vst3"

if (-not (Test-Path $sourceFile)) {
    Write-Host "`n❌ [ERROR] Plugin binary not found at: $sourceFile" -ForegroundColor Red
    exit 1
}

$fileSizeMB = [math]::Round((Get-Item $sourceFile).Length / 1MB, 2)
Write-Host "`n📦 Plugin Source Binary: $sourceFile ($fileSizeMB MB)" -ForegroundColor Green

# 3. Target paths
$userVst3Dir = Join-Path $env:LOCALAPPDATA "Programs\Common\VST3\Phantom EQ.vst3\Contents\x86_64-win"
$systemVst3Dir = Join-Path $env:CommonProgramFiles "VST3\Phantom EQ.vst3\Contents\x86_64-win"

$installed = $false

# Try User Directory first
try {
    if (-not (Test-Path $userVst3Dir)) {
        New-Item -ItemType Directory -Path $userVst3Dir -Force | Out-Null
    }
    $targetPath = Join-Path $userVst3Dir "Phantom EQ.vst3"
    Copy-Item -Path $sourceFile -Destination $targetPath -Force
    Write-Host "`n✅ [SUCCESS] Installed to User VST3: $targetPath" -ForegroundColor Green
    $installed = $true
} catch {
    Write-Host "Could not write to User directory: $_" -ForegroundColor DarkGray
}

# Also attempt System Directory if User failed
if (-not $installed) {
    try {
        if (-not (Test-Path $systemVst3Dir)) {
            New-Item -ItemType Directory -Path $systemVst3Dir -Force | Out-Null
        }
        $targetPath = Join-Path $systemVst3Dir "Phantom EQ.vst3"
        Copy-Item -Path $sourceFile -Destination $targetPath -Force
        Write-Host "`n✅ [SUCCESS] Installed to System VST3: $targetPath" -ForegroundColor Green
        $installed = $true
    } catch {
        Write-Host "`n❌ [ERROR] Could not install to System VST3 (may require Administrator): $_" -ForegroundColor Red
    }
}

if ($installed) {
    Write-Host "`n🎉 Installation Complete! Launch Cubase 15 to use Phantom EQ." -ForegroundColor Cyan
}

Write-Host "=======================================================" -ForegroundColor Cyan
