<#
.SYNOPSIS
    Builds Keva for Windows.

.DESCRIPTION
    Builds the WebView frontend (Vite/pnpm) and Rust application.

.PARAMETER Debug
    Build in debug mode (default: release).

.PARAMETER SkipFrontend
    Skip frontend build (use existing dist/).

.EXAMPLE
    ./build.ps1
    # Release build for distribution

.EXAMPLE
    ./build.ps1 -Debug
    # Debug build
#>

param(
    [switch]$Debug,
    [switch]$SkipFrontend
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "../../..")
$viteDir = Join-Path $repoRoot "keva_windows/src/webview/vite"

Write-Host "=== Keva Windows Build ===" -ForegroundColor Cyan
Write-Host "Repository: $repoRoot"
Write-Host "Mode: $(if ($Debug) { 'Debug' } else { 'Release' })"
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
    # Distribution builds use the 'dist' feature for exe-relative paths
    $cargoArgs = @("build", "-p", "keva_windows", "--features", "dist")
    if (-not $Debug) {
        $cargoArgs += "--release"
    }

    Write-Host "Running cargo $($cargoArgs -join ' ')..."
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    $profile = if ($Debug) { "debug" } else { "release" }
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
