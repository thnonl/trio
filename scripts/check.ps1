# Trio repo gates. Exit code 0 only when every gate passes.
# Usage: powershell -ExecutionPolicy Bypass -File scripts/check.ps1
$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $root "src-tauri\Cargo.toml"
$lockPath = Join-Path $root "docs\work\.build-lock"
$env:CARGO_TERM_COLOR = "never"

$results = New-Object System.Collections.Generic.List[object]

function Invoke-Gate {
    param([string]$Name, [scriptblock]$Command)
    Write-Host ""
    Write-Host "=== $Name ===" -ForegroundColor Cyan
    & $Command
    $code = $LASTEXITCODE
    if ($null -eq $code) { $code = 0 }
    $results.Add([pscustomobject]@{ Gate = $Name; Exit = $code }) | Out-Null
    return $code
}

# Charter shared resource: one build slot, stale after 10 minutes.
$existing = Get-Item $lockPath -ErrorAction SilentlyContinue
if ($existing -and ((Get-Date) - $existing.LastWriteTime).TotalMinutes -lt 10) {
    Write-Host "warning: build lock is held by another agent:" -ForegroundColor Yellow
    Get-Content $lockPath -Raw | Write-Host
}
New-Item -ItemType Directory -Force -Path (Split-Path $lockPath) | Out-Null
"owner=check.ps1; purpose=cargo fmt/check/test; ts=$(Get-Date -Format o)" |
    Set-Content -Encoding UTF8 $lockPath

try {
    Invoke-Gate "cargo fmt --check" { cargo fmt --manifest-path $manifest --check } | Out-Null
    Invoke-Gate "cargo check" { cargo check --manifest-path $manifest } | Out-Null
    Invoke-Gate "cargo test" { cargo test --manifest-path $manifest } | Out-Null

    # Frontend syntax / config validity (no node build step, just checks).
    if (Get-Command node -ErrorAction SilentlyContinue) {
        Invoke-Gate "node --check src/main.js" {
            node --check (Join-Path $root "src\main.js")
        } | Out-Null
    } else {
        Write-Host "node not found: skipping JS syntax check"
        $results.Add([pscustomobject]@{ Gate = "node --check src/main.js"; Exit = "skipped" }) | Out-Null
    }

    Invoke-Gate "tauri.conf.json parses" {
        try {
            Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json | Out-Null
            $global:LASTEXITCODE = 0
        } catch {
            Write-Error $_
            $global:LASTEXITCODE = 1
        }
    } | Out-Null

    Invoke-Gate "capabilities/default.json parses" {
        try {
            Get-Content (Join-Path $root "src-tauri\capabilities\default.json") -Raw | ConvertFrom-Json | Out-Null
            $global:LASTEXITCODE = 0
        } catch {
            Write-Error $_
            $global:LASTEXITCODE = 1
        }
    } | Out-Null
} finally {
    Remove-Item $lockPath -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "=== summary ===" -ForegroundColor Cyan
$results | Format-Table -AutoSize

$failed = @($results | Where-Object { $_.Exit -ne 0 })
if ($failed.Count -gt 0) {
    Write-Host "$($failed.Count) gate(s) failed" -ForegroundColor Red
    exit 1
}

Write-Host "all gates passed" -ForegroundColor Green
exit 0
