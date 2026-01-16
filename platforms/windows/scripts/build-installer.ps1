<#
.SYNOPSIS
    Builds the Keva MSI installer.

.DESCRIPTION
    Builds WiX installer package.
    Requires: .NET SDK (for WiX), WiX Toolset v6

.PARAMETER Version
    Version to embed in installer. If not specified, reads from Cargo.toml.

.PARAMETER Debug
    Build installer with debug executable.

.PARAMETER ExePath
    Path to keva_windows.exe. Overrides -Debug flag.

.EXAMPLE
    ./build-installer.ps1
    # Build release installer

.EXAMPLE
    ./build-installer.ps1 -Debug
    # Build debug installer
#>

param(
    [string]$Version,
    [switch]$Debug,
    [string]$ExePath
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "../../..")
$installerDir = Resolve-Path (Join-Path $PSScriptRoot "../installer")
$outputDir = Join-Path $repoRoot "dist/windows"

# Get version from Cargo.toml if not specified
if (-not $Version) {
    $Version = & (Join-Path $PSScriptRoot "extract-version.ps1")
}

# Default exe path based on Debug flag
if (-not $ExePath) {
    $profile = if ($Debug) { "debug" } else { "release" }
    $ExePath = Join-Path $repoRoot "target/$profile/keva_windows.exe"
}

if (-not (Test-Path $ExePath)) {
    $buildCmd = if ($Debug) { "./platforms/windows/scripts/build.ps1 -Debug" } else { "./platforms/windows/scripts/build.ps1" }
    Write-Error "Executable not found: $ExePath`nRun '$buildCmd' first."
    exit 1
}

$ExePath = Resolve-Path $ExePath
$buildType = if ($Debug) { "Debug" } else { "Release" }

# WebView UI dist folder (Vite build output)
$distPath = Join-Path $repoRoot "keva_windows/src/webview/vite/dist"
if (-not (Test-Path $distPath)) {
    Write-Error "Dist folder not found: $distPath`nRun 'pnpm build' in keva_windows/src/webview/vite first."
    exit 1
}
$distPath = Resolve-Path $distPath

Write-Host "=== Keva Installer Build ===" -ForegroundColor Cyan
Write-Host "Version: $Version"
Write-Host "Build: $buildType"
Write-Host "Exe: $ExePath"
Write-Host "Dist: $distPath"
Write-Host ""

# Create output directory
if (-not (Test-Path $outputDir)) {
    New-Item -ItemType Directory -Path $outputDir | Out-Null
}

# Build custom actions
Write-Host "=== Building Custom Actions ===" -ForegroundColor Cyan

$customActionsDir = Join-Path $installerDir "CustomActions"
Push-Location $customActionsDir
try {
    dotnet build -c Release -o "$outputDir/CustomActions"
    if ($LASTEXITCODE -ne 0) { throw "dotnet build failed" }
    Write-Host "Custom actions built." -ForegroundColor Green
}
finally {
    Pop-Location
}
Write-Host ""

$customActionsDll = Join-Path $outputDir "CustomActions/CustomActions.CA.dll"
if (-not (Test-Path $customActionsDll)) {
    Write-Error "Custom actions DLL not found: $customActionsDll"
    exit 1
}

# Build WiX installer
Write-Host "=== Building WiX Installer ===" -ForegroundColor Cyan

Push-Location $installerDir
try {
    $debugSuffix = if ($Debug) { "-debug" } else { "" }
    $msiName = "keva-windows-$Version-x64$debugSuffix.msi"
    $msiPath = Join-Path $outputDir $msiName

    # Build using MSBuild with WiX SDK (handles extensions via NuGet)
    dotnet build Keva.wixproj `
        -c Release `
        -p:Version=$Version `
        -p:ExePath=$ExePath `
        -p:DistPath=$distPath `
        -p:CustomActionsPath=$customActionsDll `
        -o $outputDir

    if ($LASTEXITCODE -ne 0) { throw "wix build failed" }

    # Find the built MSI and rename if needed
    $builtMsi = Get-ChildItem -Path $outputDir -Filter "*.msi" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($builtMsi -and $builtMsi.Name -ne $msiName) {
        Move-Item -Path $builtMsi.FullName -Destination $msiPath -Force
    }

    if (Test-Path $msiPath) {
        $size = (Get-Item $msiPath).Length / 1MB
        Write-Host "Installer built: $msiPath ($([math]::Round($size, 2)) MB)" -ForegroundColor Green
    } else {
        Write-Error "MSI not found after build"
        exit 1
    }
}
finally {
    Pop-Location
}

Write-Host ""
Write-Host "=== Build Successful ===" -ForegroundColor Green
Write-Host "Output: $msiPath"
