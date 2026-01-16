<#
.SYNOPSIS
    Extracts version from keva_windows/Cargo.toml

.DESCRIPTION
    Parses Cargo.toml to extract the package version.
    Outputs to $GITHUB_OUTPUT if running in GitHub Actions, otherwise prints to stdout.

.EXAMPLE
    ./extract-version.ps1
    # Output: 0.1.0

.EXAMPLE
    ./extract-version.ps1 -SetEnv
    # Sets $env:VERSION = "0.1.0"
#>

param(
    [switch]$SetEnv
)

$ErrorActionPreference = "Stop"

$cargoTomlPath = Join-Path $PSScriptRoot "../../../keva_windows/Cargo.toml"
$cargoTomlPath = Resolve-Path $cargoTomlPath

if (-not (Test-Path $cargoTomlPath)) {
    Write-Error "Cargo.toml not found at: $cargoTomlPath"
    exit 1
}

$content = Get-Content $cargoTomlPath -Raw

if ($content -match 'version\s*=\s*"([^"]+)"') {
    $version = $Matches[1]
} else {
    Write-Error "Could not find version in Cargo.toml"
    exit 1
}

# Validate semver format (MAJOR.MINOR.PATCH)
if ($version -notmatch '^\d+\.\d+\.\d+$') {
    Write-Error "Version '$version' is not valid semver (expected MAJOR.MINOR.PATCH)"
    exit 1
}

if ($SetEnv) {
    $env:VERSION = $version
    Write-Host "Set `$env:VERSION = $version"
}

# GitHub Actions output
if ($env:GITHUB_OUTPUT) {
    "version=$version" | Out-File -FilePath $env:GITHUB_OUTPUT -Append
    Write-Host "Set GitHub output: version=$version"
}

# Always print to stdout
Write-Output $version
