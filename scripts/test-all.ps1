<#
.SYNOPSIS
    Build and test all five MAGE crates.

.DESCRIPTION
    The repository is five separate Cargo workspaces on purpose (see
    ARCHITECTURE.md §"Repository layout"), so a root `cargo test` does nothing.
    This is the single entry point that covers everything CI covers:

        rmi (cpu)   1,380 tests
        prototype   1,038 tests
        ribosome      162 tests
        germline      112 tests
        forge          52 tests
        -------------------------
        total       2,744 tests, 0 warnings

.PARAMETER Release
    Build and test in release mode (slower to build, much faster to run).

.PARAMETER Bench
    Additionally run the ignored measurement harnesses: eval_bench (73/73 exact)
    and perf_report. Implies -Release, since the numbers are only meaningful
    against an optimized build.

.PARAMETER Cuda
    Additionally test the prototype with --features cuda (1,269 tests). Needs an
    NVIDIA driver to exercise the kernels; without one the CUDA backend falls
    back to CPU and the suite still passes. CI only compile-checks this path.

.EXAMPLE
    ./scripts/test-all.ps1
.EXAMPLE
    ./scripts/test-all.ps1 -Bench -Cuda
#>
[CmdletBinding()]
param(
    [switch]$Release,
    [switch]$Bench,
    [switch]$Cuda
)

$ErrorActionPreference = 'Stop'
if ($Bench) { $Release = $true }

$repo = Split-Path -Parent $PSScriptRoot
$profileArgs = if ($Release) { @('--release') } else { @() }

# rmi is feature-gated: the default feature set pulls in GPU backends that need
# a toolchain CI does not have, so `cpu` is the portable, always-buildable one.
$crates = @(
    @{ Name = 'rmi';       Manifest = 'RecursiveMachineIntelligence/Cargo.toml'; Features = @('--no-default-features', '--features', 'cpu') }
    @{ Name = 'prototype'; Manifest = 'prototype/Cargo.toml';                    Features = @() }
    @{ Name = 'ribosome';  Manifest = 'ribosome/Cargo.toml';                     Features = @() }
    @{ Name = 'germline';  Manifest = 'germline/Cargo.toml';                     Features = @() }
    @{ Name = 'forge';     Manifest = 'forge/Cargo.toml';                        Features = @() }
)

$failed = @()
foreach ($c in $crates) {
    Write-Host "`n=== $($c.Name) ===" -ForegroundColor Cyan
    $manifest = Join-Path $repo $c.Manifest
    & cargo test --manifest-path $manifest @profileArgs @($c.Features)
    if ($LASTEXITCODE -ne 0) { $failed += $c.Name }
}

if ($Cuda) {
    Write-Host "`n=== prototype (cuda) ===" -ForegroundColor Cyan
    & cargo test --manifest-path (Join-Path $repo 'prototype/Cargo.toml') @profileArgs --features cuda
    if ($LASTEXITCODE -ne 0) { $failed += 'prototype (cuda)' }
}

if ($Bench) {
    Write-Host "`n=== measurement harnesses ===" -ForegroundColor Cyan
    $proto = Join-Path $repo 'prototype/Cargo.toml'
    foreach ($harness in @('eval_bench', 'perf_report')) {
        & cargo test --manifest-path $proto --release $harness -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { $failed += "prototype::$harness" }
    }
}

Write-Host ''
if ($failed.Count -gt 0) {
    Write-Host "FAILED: $($failed -join ', ')" -ForegroundColor Red
    exit 1
}
Write-Host 'All crates green.' -ForegroundColor Green
