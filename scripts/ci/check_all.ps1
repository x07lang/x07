Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Exec([string]$File, [string[]]$Args) {
  $psi = New-Object System.Diagnostics.ProcessStartInfo
  $psi.FileName = $File
  $psi.Arguments = ($Args -join " ")
  $psi.UseShellExecute = $false
  $psi.RedirectStandardOutput = $false
  $psi.RedirectStandardError = $false
  $p = New-Object System.Diagnostics.Process
  $p.StartInfo = $psi
  [void]$p.Start()
  $p.WaitForExit()
  if ($p.ExitCode -ne 0) {
    throw "$File exited with code $($p.ExitCode)"
  }
}

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\\..")).Path
Set-Location $root

if (-not (Get-Command bash -ErrorAction SilentlyContinue)) {
  Write-Error "bash not found; on Windows run from WSL2 (then execute ./scripts/ci/check_all.sh)."
}

Exec "bash" @("-lc", "./scripts/ci/check_all.sh")
