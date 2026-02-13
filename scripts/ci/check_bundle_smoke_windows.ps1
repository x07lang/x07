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

function Assert-StdoutOk([string]$stdoutBin, [string]$fixture, [string]$profile) {
  $bytes = [System.IO.File]::ReadAllBytes($stdoutBin)
  $expected = [System.Text.Encoding]::UTF8.GetBytes("ok")
  if ($bytes.Length -ne $expected.Length) {
    throw "stdout mismatch fixture=${fixture} profile=${profile}: got_len=$($bytes.Length) expected_len=$($expected.Length)"
  }
  for ($i = 0; $i -lt $expected.Length; $i++) {
    if ($bytes[$i] -ne $expected[$i]) {
      $got = [System.Text.Encoding]::UTF8.GetString($bytes)
      throw "stdout mismatch fixture=${fixture} profile=${profile}: got_bytes=$($bytes -join ',') got_text='$got'"
    }
  }
}

Need "python"
Need "cargo"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $root

$x07 = Find-X07 $root

$tmp = $env:OUTDIR
if ([string]::IsNullOrWhiteSpace($tmp)) {
  $tmp = Join-Path $root ".x07\ci-out\bundle-smoke"
}

New-Item -ItemType Directory -Force -Path $tmp | Out-Null

$keep = $env:X07_BUNDLE_SMOKE_KEEP_TMP
try {
  $fixtures = @(
    @{ Name = "echo-argv"; Template = "cli"; Expect = "argv_v1"; PolicyRel = ".x07\\policies\\base\\cli.sandbox.base.policy.json"; NeedsHelper = $false },
    @{ Name = "async-only"; Template = "worker"; Expect = "ok"; PolicyRel = ".x07\\policies\\base\\worker.sandbox.base.policy.json"; NeedsHelper = $false },
    @{ Name = "process-async-join"; Template = "worker-parallel"; Expect = "ok"; PolicyRel = ".x07\\policies\\base\\worker-parallel.sandbox.base.policy.json"; NeedsHelper = $true }
  )

  $helperSrc = $null
  if ($fixtures | Where-Object { $_.NeedsHelper }) {
    Step "build os helper (x07-proc-echo)"
    & cargo build --release -p x07-proc-echo | Out-Host
    $helperSrc = Join-Path $root "target\\release\\x07-proc-echo.exe"
    if (-not (Test-Path $helperSrc)) {
      throw "missing os helper build output: $helperSrc"
    }
  }

  $profiles = @("test","os","sandbox")
  foreach ($fx in $fixtures) {
    $fixtureName = $fx.Name
    $fixtureSrc = Join-Path $root ("ci\\fixtures\\bundle\\" + $fixtureName)
    if (-not (Test-Path $fixtureSrc)) {
      throw "missing fixture dir: $fixtureSrc"
    }

    $fixture = Join-Path $tmp $fixtureName
    Copy-Item -Recurse -Force $fixtureSrc $fixture

    Set-Location $fixture

    Step "policy init (sandbox base policy): fixture=$fixtureName template=$($fx.Template)"
    $policyReport = Join-Path $tmp ("policy.init." + $fixtureName + ".json")
    & $x07 policy init --template $fx.Template --project x07.json --emit report | Out-File -Encoding utf8 $policyReport

    $policyPath = Join-Path $fixture $fx.PolicyRel
    if (-not (Test-Path $policyPath)) {
      throw "policy init did not create expected file: $policyPath"
    }

    foreach ($profile in $profiles) {
      $outdir = Join-Path $tmp ("out\\" + $fixtureName + "\\" + $profile)
      New-Item -ItemType Directory -Force -Path $outdir | Out-Null

      $outbin = Join-Path $outdir ($fixtureName + ".exe")
      $emitdir = Join-Path $outdir "emit"
      New-Item -ItemType Directory -Force -Path $emitdir | Out-Null
      $report = Join-Path $outdir "bundle.report.json"

      Step "bundle: fixture=$fixtureName profile=$profile"
      # `x07 bundle` must print x07.bundle.report@0.2.0 JSON to stdout (machine-clean).
      $json = & $x07 bundle --project x07.json --profile $profile --out $outbin --emit-dir $emitdir
      $json | Out-File -Encoding utf8 $report

      if (-not (Test-Path $outbin)) {
        throw "bundle did not produce expected binary: $outbin (see $report)"
      }

      if ($fx.NeedsHelper) {
        $depsDir = Join-Path $outdir "deps\\x07"
        New-Item -ItemType Directory -Force -Path $depsDir | Out-Null
        Copy-Item -Force $helperSrc (Join-Path $depsDir "x07-proc-echo.exe")
        Copy-Item -Force $helperSrc (Join-Path $depsDir "x07-proc-echo")
      }

      Step "run bundled binary via cmd.exe (binary-safe stdout redirection): fixture=$fixtureName profile=$profile"
      $runDir = Join-Path $outdir "run"
      New-Item -ItemType Directory -Force -Path $runDir | Out-Null
      Copy-Item -Force $outbin (Join-Path $runDir ($fixtureName + ".exe"))
      if (Test-Path (Join-Path $outdir "deps")) {
        Copy-Item -Recurse -Force (Join-Path $outdir "deps") (Join-Path $runDir "deps")
      }

      $stdoutBin = Join-Path $outdir "stdout.bin"
      $stderrTxt = Join-Path $outdir "stderr.txt"
      Remove-Item -Force $stdoutBin -ErrorAction SilentlyContinue
      Remove-Item -Force $stderrTxt -ErrorAction SilentlyContinue

      $cmdLine = "cd /d `"$runDir`" && `".\\$fixtureName.exe`" --alpha A --beta B > `"$stdoutBin`" 2> `"$stderrTxt`""
      cmd.exe /c $cmdLine | Out-Null
      if ($LASTEXITCODE -ne 0) {
        $stderr = ""
        if (Test-Path $stderrTxt) { $stderr = (Get-Content $stderrTxt -Raw) }
        throw "bundled binary failed (exit=$LASTEXITCODE) fixture=$fixtureName profile=$profile`n$stderr"
      }

      if (-not (Test-Path $stdoutBin)) {
        throw "missing stdout capture: $stdoutBin"
      }

      if ($fx.Expect -eq "argv_v1") {
        Step "validate argv_v1 output: fixture=$fixtureName profile=$profile"
        $bytes = [System.IO.File]::ReadAllBytes($stdoutBin)
        $args = Parse-ArgvV1 $bytes

        $expected = @("echo-argv","--alpha","A","--beta","B")
        if ($args.Count -ne $expected.Count) {
          throw "argv_v1 arg count mismatch fixture=$fixtureName profile=${profile}: got=$($args.Count) expected=$($expected.Count) args=$($args -join ' | ')"
        }
        for ($i = 0; $i -lt $expected.Count; $i++) {
          if ($args[$i] -ne $expected[$i]) {
            throw "argv_v1 mismatch fixture=$fixtureName profile=$profile at index=${i}: got='$($args[$i])' expected='$($expected[$i])' all=$($args -join ' | ')"
          }
        }
        Write-Host "ok: argv_v1 ($fixtureName::$profile)"
      } else {
        Step "validate stdout 'ok': fixture=$fixtureName profile=$profile"
        Assert-StdoutOk $stdoutBin $fixtureName $profile
        Write-Host "ok: stdout 'ok' ($fixtureName::$profile)"
      }
    }

    Set-Location $root
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
