Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Step([string]$msg) {
  Write-Host ""
  Write-Host "==> $msg"
}

function Need([string]$cmd) {
  if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
    throw "missing tool: $cmd"
  }
}

function Find-X07([string]$root) {
  $override = $env:X07_BIN
  if (-not [string]::IsNullOrWhiteSpace($override) -and (Test-Path $override)) {
    return (Resolve-Path $override).Path
  }

  $candidates = @(
    (Join-Path $root "target\release\x07.exe"),
    (Join-Path $root "target\debug\x07.exe"),
    (Join-Path $root "target\release\x07"),
    (Join-Path $root "target\debug\x07")
  )
  foreach ($c in $candidates) {
    if (Test-Path $c) { return (Resolve-Path $c).Path }
  }

  Step "build x07 (release)"
  & cargo build --release -p x07 | Out-Host

  foreach ($c in $candidates) {
    if (Test-Path $c) { return (Resolve-Path $c).Path }
  }

  throw "x07 binary not found under target/{debug,release}/ (set X07_BIN or build -p x07)"
}

function Parse-ArgvV1([byte[]]$bytes) {
  if ($bytes.Length -lt 4) {
    throw "argv_v1 too short: $($bytes.Length) bytes"
  }

  $argc = [BitConverter]::ToUInt32($bytes, 0)
  $off = 4

  $args = New-Object System.Collections.Generic.List[string]
  for ($i = 0; $i -lt $argc; $i++) {
    if ($off + 4 -gt $bytes.Length) {
      throw "argv_v1 truncated at arg $i (missing len)"
    }
    $n = [BitConverter]::ToUInt32($bytes, $off)
    $off += 4
    if ($off + $n -gt $bytes.Length) {
      throw "argv_v1 truncated at arg $i (need $n bytes)"
    }
    $argBytes = $bytes[$off..($off + $n - 1)]
    $off += $n
    $args.Add([System.Text.Encoding]::UTF8.GetString($argBytes))
  }

  if ($off -ne $bytes.Length) {
    throw "argv_v1 has trailing bytes: parsed=$off total=$($bytes.Length)"
  }

  return $args
}

Need "python"
Need "cargo"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $root

$x07 = Find-X07 $root

$fixtureSrc = Join-Path $root "ci\fixtures\bundle\echo-argv"
if (-not (Test-Path $fixtureSrc)) {
  throw "missing fixture dir: $fixtureSrc"
}

$tmp = $env:OUTDIR
if ([string]::IsNullOrWhiteSpace($tmp)) {
  $tmp = Join-Path $root ".x07\ci-out\bundle-smoke"
}

New-Item -ItemType Directory -Force -Path $tmp | Out-Null

$keep = $env:X07_BUNDLE_SMOKE_KEEP_TMP
try {
  $fixture = Join-Path $tmp "echo-argv"
  Copy-Item -Recurse -Force $fixtureSrc $fixture

  Set-Location $fixture

  Step "policy init (sandbox base policy)"
  $policyReport = Join-Path $tmp "policy.init.report.json"
  & $x07 policy init --template cli --project x07.json --emit report | Out-File -Encoding utf8 $policyReport

  $policyPath = Join-Path $fixture ".x07\policies\base\cli.sandbox.base.policy.json"
  if (-not (Test-Path $policyPath)) {
    throw "policy init did not create expected file: $policyPath"
  }

  $profiles = @("test","os","sandbox")
  foreach ($profile in $profiles) {
    $outdir = Join-Path $tmp ("out\" + $profile)
    New-Item -ItemType Directory -Force -Path $outdir | Out-Null

    $outbin = Join-Path $outdir "echo-argv.exe"
    $emitdir = Join-Path $outdir "emit"
    New-Item -ItemType Directory -Force -Path $emitdir | Out-Null
    $report = Join-Path $outdir "bundle.report.json"

    Step "bundle: profile=$profile"
    # `x07 bundle` must print x07.bundle.report@0.1.0 JSON to stdout (machine-clean).
    $json = & $x07 bundle --project x07.json --profile $profile --out $outbin --emit-dir $emitdir
    $json | Out-File -Encoding utf8 $report

    if (-not (Test-Path $outbin)) {
      throw "bundle did not produce expected binary: $outbin (see $report)"
    }

    Step "run bundled binary via cmd.exe (binary-safe stdout redirection): profile=$profile"
    $runDir = Join-Path $outdir "run"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null
    Copy-Item -Force $outbin (Join-Path $runDir "echo-argv.exe")

    $stdoutBin = Join-Path $outdir "stdout.bin"
    $stderrTxt = Join-Path $outdir "stderr.txt"
    Remove-Item -Force $stdoutBin -ErrorAction SilentlyContinue
    Remove-Item -Force $stderrTxt -ErrorAction SilentlyContinue

    $cmdLine = "cd /d `"$runDir`" && `".\echo-argv.exe`" --alpha A --beta B > `"$stdoutBin`" 2> `"$stderrTxt`""
    cmd.exe /c $cmdLine | Out-Null
    if ($LASTEXITCODE -ne 0) {
      $stderr = ""
      if (Test-Path $stderrTxt) { $stderr = (Get-Content $stderrTxt -Raw) }
      throw "bundled binary failed (exit=$LASTEXITCODE) profile=$profile`n$stderr"
    }

    if (-not (Test-Path $stdoutBin)) {
      throw "missing stdout capture: $stdoutBin"
    }

    Step "validate argv_v1 output: profile=$profile"
    $bytes = [System.IO.File]::ReadAllBytes($stdoutBin)
    $args = Parse-ArgvV1 $bytes

    $expected = @("echo-argv","--alpha","A","--beta","B")
    if ($args.Count -ne $expected.Count) {
      throw "argv_v1 arg count mismatch profile=$profile: got=$($args.Count) expected=$($expected.Count) args=$($args -join ' | ')"
    }
    for ($i = 0; $i -lt $expected.Count; $i++) {
      if ($args[$i] -ne $expected[$i]) {
        throw "argv_v1 mismatch profile=$profile at index=$i: got='$($args[$i])' expected='$($expected[$i])' all=$($args -join ' | ')"
      }
    }

    Write-Host "ok: argv_v1 ($profile)"
  }

  Write-Host ""
  Write-Host "ok: bundle smoke (windows) passed"
} finally {
  if ([string]::IsNullOrWhiteSpace($keep)) {
    try { Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue } catch {}
  } else {
    Write-Host "[bundle-smoke] kept tmp dir: $tmp"
  }
}
