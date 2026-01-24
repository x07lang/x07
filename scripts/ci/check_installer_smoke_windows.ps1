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

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $root

Need "python"
Need "curl"

$mode = $env:X07_INSTALL_SMOKE_MODE
if ([string]::IsNullOrWhiteSpace($mode)) { $mode = "local" }

$installRoot = Join-Path $env:TEMP "x07root_$([Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Force -Path $installRoot | Out-Null

$installer = $env:X07_INSTALLER_PS1
if ([string]::IsNullOrWhiteSpace($installer)) {
  $local = Join-Path $root "dist\install\install.ps1"
  if (Test-Path $local) { $installer = $local } else { $installer = "https://x07lang.org/install.ps1" }
}

$channelsUrl = $env:X07_CHANNELS_URL
if ([string]::IsNullOrWhiteSpace($channelsUrl)) { $channelsUrl = "https://x07lang.org/install/channels.json" }

$serverProc = $null
$installerTemp = $null
try {
  if ($mode -eq "local") {
    Need "cargo"

    $target = "x86_64-pc-windows-msvc"
    $tag = "v0.0.0-ci"
    $artifacts = Join-Path $env:TEMP "x07_artifacts_$([Guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Force -Path $artifacts | Out-Null

    Step "build release binaries (including x07up)"
    cargo build --release -p x07 -p x07c -p x07-host-runner -p x07-os-runner -p x07import-cli -p x07up

    Step "package x07up archive"
    $x07upStage = Join-Path $env:TEMP "x07up_stage_$([Guid]::NewGuid().ToString('N'))"
    Remove-Item -Recurse -Force $x07upStage -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $x07upStage | Out-Null
    Copy-Item -Force "target/release/x07up.exe" (Join-Path $x07upStage "x07up.exe")
    $x07upArchive = Join-Path $artifacts "x07up-$tag-$target.zip"
    Compress-Archive -Path (Join-Path $x07upStage "*") -DestinationPath $x07upArchive -Force

    Step "package toolchain archive (CI minimal)"
    $toolchainStage = Join-Path $env:TEMP "x07_toolchain_stage_$([Guid]::NewGuid().ToString('N'))"
    Remove-Item -Recurse -Force $toolchainStage -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path (Join-Path $toolchainStage "bin") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $toolchainStage "deps\\x07") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $toolchainStage "stdlib\\os\\0.2.0") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $toolchainStage "docs") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $toolchainStage ".codex\\skills") | Out-Null

    $bins = @("x07","x07c","x07-host-runner","x07-os-runner","x07import-cli")
    foreach ($b in $bins) {
      Copy-Item -Force "target/release/$b.exe" (Join-Path $toolchainStage "bin\\$b.exe")
    }

    Copy-Item -Force "deps/x07/native_backends.json" (Join-Path $toolchainStage "deps\\x07\\native_backends.json")
    Copy-Item -Recurse -Force "stdlib/os/0.2.0/modules" (Join-Path $toolchainStage "stdlib\\os\\0.2.0")
    Copy-Item -Force "stdlib.lock" (Join-Path $toolchainStage "stdlib.lock")
    Copy-Item -Force "stdlib.os.lock" (Join-Path $toolchainStage "stdlib.os.lock")
    Copy-Item -Recurse -Force "docs/*" (Join-Path $toolchainStage "docs")
    Copy-Item -Recurse -Force "skills/pack/.codex/skills/*" (Join-Path $toolchainStage ".codex\\skills")

    $toolchainArchive = Join-Path $artifacts "x07-$tag-$target.zip"
    Compress-Archive -Path (Join-Path $toolchainStage "*") -DestinationPath $toolchainArchive -Force

    Step "start local artifacts server"
    $serverJson = Join-Path $env:TEMP "x07_server_$([Guid]::NewGuid().ToString('N')).json"
    Remove-Item -Force $serverJson -ErrorAction SilentlyContinue
    $serverProc = Start-Process -PassThru -NoNewWindow -FilePath "python" -ArgumentList @("scripts/ci/local_http_server.py","--root",$artifacts,"--ready-json",$serverJson,"--quiet")

    for ($i = 0; $i -lt 100; $i++) {
      if (Test-Path $serverJson) { break }
      Start-Sleep -Milliseconds 50
    }
    if (-not (Test-Path $serverJson)) { throw "local server did not publish ready json" }
    $serverInfo = Get-Content $serverJson -Raw | ConvertFrom-Json
    $baseUrl = [string]$serverInfo.url
    $baseUrl = $baseUrl.TrimEnd("/")

    Step "write local channels.json"
    python scripts/ci/make_channels_json.py `
      --base-url "$baseUrl" `
      --out "$artifacts/channels.json" `
      --tag "$tag" `
      --target "$target" `
      --toolchain-file "$toolchainArchive" `
      --x07up-file "$x07upArchive"

    $channelsUrl = "$baseUrl/channels.json"
    $installer = Join-Path $root "dist\install\install.ps1"
  }

  Step "run installer (mode=$mode)"
  $installReport = Join-Path $env:TEMP "x07_install_report_$([Guid]::NewGuid().ToString('N')).json"

  $installerPath = $installer
  if ($installerPath.StartsWith("http")) {
    Step "download installer script"
    $installerTemp = Join-Path $env:TEMP "x07_install_$([Guid]::NewGuid().ToString('N')).ps1"
    $scriptText = (Invoke-WebRequest -UseBasicParsing $installerPath).Content
    $scriptText | Out-File -Encoding utf8 $installerTemp
    $installerPath = $installerTemp
  }

  powershell -NoProfile -ExecutionPolicy Bypass -File $installerPath `
    -Yes `
    -Root "$installRoot" `
    -Channel "stable" `
    -ChannelsUrl "$channelsUrl" `
    -NoModifyPath `
    -Json | Out-File -Encoding utf8 $installReport

  $env:PATH = (Join-Path $installRoot "bin") + ";" + $env:PATH

  Step "smoke: x07up show"
  $x07upShow = Join-Path $env:TEMP "x07up_show_$([Guid]::NewGuid().ToString('N')).json"
  x07up show --json | Out-File -Encoding utf8 $x07upShow
  $doc = Get-Content $x07upShow -Raw | ConvertFrom-Json
  foreach ($k in @("schema_version","toolchains","active","channels")) {
    if (-not ($doc.PSObject.Properties.Name -contains $k)) { throw "x07up show missing key: $k" }
  }

  Step "smoke: x07 help"
  x07 --help | Out-Null

  Step "smoke: init+run (host profile)"
  $proj = Join-Path $env:TEMP "x07proj_$([Guid]::NewGuid().ToString('N'))"
  New-Item -ItemType Directory -Force -Path $proj | Out-Null
  Set-Location $proj

  x07 init | Out-Null
  "hello" | Set-Content -NoNewline -Encoding ascii "input.bin"

  New-Item -ItemType Directory -Force -Path (Join-Path $proj ".x07") | Out-Null
  x07 run --profile test --input input.bin --report wrapped --report-out .x07\run.host.json | Out-Null

  $runHost = Get-Content ".x07\run.host.json" -Raw | ConvertFrom-Json
  if ($runHost.schema_version -ne "x07.run.report@0.1.0") { throw "wrapped report schema_version mismatch" }
  if ($runHost.runner -ne "host") { throw "expected runner=host" }
  $rep = $runHost.report
  if ($null -eq $rep) { throw "missing report object" }
  if ($rep.exit_code -ne 0) { throw "host run exit_code != 0" }
  if ($rep.compile.ok -ne $true) { throw "host compile not ok" }
  if ($rep.solve.ok -ne $true) { throw "host solve not ok" }

  $roots = @()
  if ($runHost.target -and $runHost.target.resolved_module_roots) { $roots = $runHost.target.resolved_module_roots }
  $hasSrc = $false
  foreach ($r in $roots) {
    $norm = ([string]$r).Replace("\\","/").Trim().TrimEnd("/")
    if ($norm.EndsWith("/src") -or $norm -eq "src") { $hasSrc = $true }
  }
  if (-not $hasSrc) { throw "expected src in resolved_module_roots" }

  Step "smoke: agent init produces AGENT.md"
  x07up agent init --project $proj --with-skills project | Out-Null
  if (-not (Test-Path (Join-Path $proj "AGENT.md"))) { throw "AGENT.md not created" }

  Write-Host ""
  Write-Host "ok: windows installer smoke passed"
} finally {
  if ($serverProc) {
    try { Stop-Process -Id $serverProc.Id -Force } catch {}
  }
  if ($installerTemp) {
    try { Remove-Item -Force $installerTemp -ErrorAction SilentlyContinue } catch {}
  }
}
