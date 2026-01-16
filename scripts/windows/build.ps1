<#
.SYNOPSIS
    Builds Keva for Windows.

.DESCRIPTION
    Builds the WebView frontend (Vite/pnpm) and Rust application.

.PARAMETER Release
    Build in release mode (default: debug).

.PARAMETER SkipFrontend
    Skip frontend build (use existing dist/).

.EXAMPLE
    ./build.ps1
    # Debug build

.EXAMPLE
    ./build.ps1 -Release
    # Release build for distribution
#>

param(
    [switch]$Release,
    [switch]$SkipFrontend
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "../..")
$viteDir = Join-Path $repoRoot "keva_windows/src/webview/vite"

Write-Host "=== Keva Windows Build ===" -ForegroundColor Cyan
Write-Host "Repository: $repoRoot"
Write-Host "Mode: $(if ($Release) { 'Release' } else { 'Debug' })"
Write-Host ""

# Step 1: Build frontend
if (-not $SkipFrontend) {
    Write-Host "=== Building Frontend ===" -ForegroundColor Cyan

    Push-Location $viteDir
    try {
        Write-Host "Running pnpm install..."
        pnpm install --frozen-lockfile
        if ($LASTEXITCODE -ne 0) { throw "pnpm install failed" }

        Write-Host "Running pnpm build..."
        pnpm build
        if ($LASTEXITCODE -ne 0) { throw "pnpm build failed" }

        Write-Host "Frontend build complete." -ForegroundColor Green
    }
    finally {
        Pop-Location
    }
    Write-Host ""
}

# Step 2: Build Rust application
Write-Host "=== Building Rust Application ===" -ForegroundColor Cyan

Push-Location $repoRoot
try {
    $cargoArgs = @("build", "-p", "keva_windows")
    if ($Release) {
        $cargoArgs += "--release"
    }

    Write-Host "Running cargo $($cargoArgs -join ' ')..."
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    $profile = if ($Release) { "release" } else { "debug" }
    $exePath = Join-Path $repoRoot "target/$profile/keva_windows.exe"

    if (Test-Path $exePath) {
        $size = (Get-Item $exePath).Length / 1MB
        Write-Host "Build complete: $exePath ($([math]::Round($size, 2)) MB)" -ForegroundColor Green
    }
}
finally {
    Pop-Location
}

Write-Host ""
Write-Host "=== Build Successful ===" -ForegroundColor Green
