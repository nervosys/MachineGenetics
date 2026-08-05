<#
.SYNOPSIS
    Build and test all five MAGE crates.

.DESCRIPTION
    The repository is five separate Cargo workspaces on purpose (see
    ARCHITECTURE.md §"Repository layout"), so a root `cargo test` does nothing.
    This is the single entry point that covers everything CI covers:

        rmi (cpu)   1,380 tests
        prototype   1,054 tests
        ribosome      164 tests
        germline      112 tests
        forge          52 tests
        -------------------------
        total       2,762 tests, 0 warnings

.PARAMETER Release
    Build and test in release mode (slower to build, much faster to run).

.PARAMETER Bench
    Additionally run the ignored measurement harnesses: eval_bench (73/73 exact)
    and perf_report. Implies -Release, since the numbers are only meaningful
    against an optimized build.

.PARAMETER CheckDocs
    After a green run, verify that every documented test count matches what the
    suites just reported. Four documented figures were found stale on
    2026-08-04, each by accident; this makes that a failure instead of a
    discovery. Delegates to scripts/check-doc-counts.sh so there is one
    implementation of the check — needs the `bash` that ships with Git.

.PARAMETER Cuda
    Additionally test the prototype with --features cuda (1,071 tests). Needs an
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
    [switch]$Cuda,
    [switch]$CheckDocs
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
$counts = @{}
foreach ($c in $crates) {
    Write-Host "`n=== $($c.Name) ===" -ForegroundColor Cyan
    $manifest = Join-Path $repo $c.Manifest
    # Tee to a file so -CheckDocs verifies against the run just displayed rather
    # than a second measurement that could disagree with it. Tee-Object rather
    # than Write-Host in a ForEach: Write-Host writes straight to the host, so it
    # bypasses the caller's own pipeline and floods any filtering they applied.
    $log = [System.IO.Path]::GetTempFileName()
    & cargo test --manifest-path $manifest @profileArgs @($c.Features) 2>&1 |
        Tee-Object -FilePath $log
    if ($LASTEXITCODE -ne 0) { $failed += $c.Name }
    $sum = (Select-String -Path $log -Pattern '^test result: ok\. (\d+) passed' |
        ForEach-Object { [int]$_.Matches[0].Groups[1].Value } |
        Measure-Object -Sum).Sum
    $counts[$c.Name] = if ($null -eq $sum) { 0 } else { $sum }
    Remove-Item $log -ErrorAction SilentlyContinue
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

if ($CheckDocs -and $failed.Count -eq 0) {
    Write-Host ''
    $total = ($counts.Values | Measure-Object -Sum).Sum
    $lines = @($counts.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }) + "total=$total"
    # One implementation of the check, in bash, rather than two that can drift.
    #
    # Two Windows traps here, both hit while writing this. `bash` on PATH is
    # WSL's, which cannot open a `C:/...` path at all; and passing a Windows
    # path with backslashes makes bash read them as escapes, yielding
    # `C:UsersadammdevMechGen...` and a bare 127. Prefer the bash that ships
    # with Git — the one these .sh scripts are written for — and invoke it with
    # a path relative to the repo, which both bashes resolve correctly.
    $bash = @(
        "$env:ProgramFiles\Git\bin\bash.exe",
        "${env:ProgramFiles(x86)}\Git\bin\bash.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $bash) { $bash = (Get-Command bash -ErrorAction SilentlyContinue).Source }

    if (-not $bash) {
        # Not a skip. A check that quietly does nothing when its interpreter is
        # missing is how the stale-doc problem got here in the first place.
        Write-Host 'CheckDocs needs bash (ships with Git for Windows).' -ForegroundColor Red
        $failed += 'documented counts (no bash available)'
    } else {
        Push-Location $repo
        try {
            $lines -join "`n" | & $bash 'scripts/check-doc-counts.sh'
            if ($LASTEXITCODE -ne 0) { $failed += 'documented counts' }
        } finally { Pop-Location }
    }
}

Write-Host ''
if ($failed.Count -gt 0) {
    Write-Host "FAILED: $($failed -join ', ')" -ForegroundColor Red
    exit 1
}
Write-Host 'All crates green.' -ForegroundColor Green
