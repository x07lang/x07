param(
  [switch]$Help,
  [switch]$Yes,
  [string]$Root = "$env:USERPROFILE\.x07",
  [string]$Channel = "stable",
  [string]$Toolchain = "",
  [string]$ChannelsUrl = "https://x07lang.org/install/channels.json",
  [ValidateSet("full","minimal")][string]$Profile = "full",
  [switch]$NoModifyPath,
  [switch]$Quiet,
  [switch]$Json
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$pinnedToolchain = -not [string]::IsNullOrWhiteSpace($Toolchain)

function Log([string]$msg) {
  if ($Quiet) { return }
  [Console]::Error.WriteLine($msg)
}

function Fail([int]$exitCode, [string]$code, [string]$message, [string]$hint = "") {
  $report = @{
    schema_version = "x07.install.report@0.1.0"
    ok = $false
    root = $Root
    bin_dir = (Join-Path $Root "bin")
    channels_url = $ChannelsUrl
    channel = $Channel
    toolchain = if ($Toolchain) { $Toolchain } else { $null }
    x07up = $null
    profile = $Profile
    target = "x86_64-pc-windows-msvc"
    modified_path = $false
    path_hint = "`$env:PATH = `"$((Join-Path $Root "bin"));`$env:PATH`""
    installed = $null
    warnings = @()
    error = @{
      code = $code
      message = $message
      hint = if ($hint) { $hint } else { $null }
    }
  }
  if ($Json) {
    $report | ConvertTo-Json -Depth 6
  } else {
    [Console]::Error.WriteLine(("{0}: {1}" -f $code, $message))
    if ($hint) { [Console]::Error.WriteLine(("hint: {0}" -f $hint)) }
  }
  exit $exitCode
}

if ($Help) {
  Write-Host "Usage: install.ps1 [-Yes] [-Root <path>] [-Channel stable] [-Toolchain vX.Y.Z] [-ChannelsUrl <url>] [-Profile full|minimal] [-NoModifyPath] [-Quiet] [-Json]"
  exit 0
}

$target = "x86_64-pc-windows-msvc"
if ($env:PROCESSOR_ARCHITECTURE -ne "AMD64") {
  Fail 2 "ASSET_NOT_FOUND" "unsupported Windows arch" $env:PROCESSOR_ARCHITECTURE
}

Log "fetch channels manifest: $ChannelsUrl"
try {
  if ((Get-Command Invoke-WebRequest).Parameters.ContainsKey("UseBasicParsing")) {
    $manifestText = (Invoke-WebRequest -UseBasicParsing $ChannelsUrl).Content
  } else {
    $manifestText = (Invoke-WebRequest $ChannelsUrl).Content
  }
  $manifest = $manifestText | ConvertFrom-Json
} catch {
  Fail 10 "NETWORK_ERROR" "failed to download channels manifest" $_.Exception.Message
}

if ($manifest.schema_version -ne "x07.install.channels@0.1.0") {
  Fail 10 "NETWORK_ERROR" "unsupported channels.json schema_version" "$($manifest.schema_version)"
}

if (-not $Toolchain) {
  if (-not ($manifest.channels.PSObject.Properties.Name -contains $Channel)) {
    Fail 2 "CHANNEL_NOT_FOUND" "channel not found: $Channel"
  }
  $Toolchain = $manifest.channels.$Channel.toolchain
}

if (-not ($manifest.toolchains.PSObject.Properties.Name -contains $Toolchain)) {
  Fail 2 "TOOLCHAIN_NOT_FOUND" "toolchain not found: $Toolchain"
}

$x07upTag = $manifest.toolchains.$Toolchain.min_required.x07up
if (-not $x07upTag) {
  $x07upTag = $manifest.channels.$Channel.x07up
}
if (-not ($manifest.x07up.PSObject.Properties.Name -contains $x07upTag)) {
  Fail 2 "X07UP_NOT_FOUND" "x07up release not found: $x07upTag"
}

$x07upAssets = $manifest.x07up.$x07upTag.assets
if (-not ($x07upAssets.PSObject.Properties.Name -contains $target)) {
  Fail 2 "ASSET_NOT_FOUND" "no x07up asset for detected target" "target=$target"
}

$asset = $x07upAssets.$target
$x07upUrl = $asset.url
$x07upSha = $asset.sha256
$x07upFmt = $asset.format

if (-not $x07upFmt) { $x07upFmt = "zip" }

if ($x07upFmt -ne "zip") {
  Fail 12 "EXTRACT_FAILED" "unsupported x07up archive format on Windows" $x07upFmt
}

$downloads = Join-Path $Root "cache\\downloads"
New-Item -ItemType Directory -Force -Path $downloads | Out-Null
$archiveName = [IO.Path]::GetFileName(([Uri]$x07upUrl).AbsolutePath)
if (-not $archiveName) { $archiveName = "x07up-$x07upTag-$target.zip" }
$x07upArchive = Join-Path $downloads $archiveName

Log "download x07up: $x07upUrl"
try {
  if ((Get-Command Invoke-WebRequest).Parameters.ContainsKey("UseBasicParsing")) {
    Invoke-WebRequest -UseBasicParsing $x07upUrl -OutFile $x07upArchive
  } else {
    Invoke-WebRequest $x07upUrl -OutFile $x07upArchive
  }
} catch {
  Fail 10 "NETWORK_ERROR" "failed to download x07up archive" $_.Exception.Message
}

$actualSha = (Get-FileHash -Algorithm SHA256 $x07upArchive).Hash.ToLower()
if ($actualSha -ne $x07upSha.ToLower()) {
  Fail 11 "SHA256_MISMATCH" "sha256 mismatch for x07up archive" "expected=$x07upSha got=$actualSha"
}

Log "extract x07up"
$tmp = Join-Path $env:TEMP "x07up_extract_$([Guid]::NewGuid().ToString('N'))"
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
  Expand-Archive -Force -Path $x07upArchive -DestinationPath $tmp
} catch {
  Fail 12 "EXTRACT_FAILED" "failed to extract x07up archive" $_.Exception.Message
}

$found = Get-ChildItem -Recurse -File -Path $tmp | Where-Object { $_.Name -ieq "x07up.exe" } | Select-Object -First 1
if (-not $found) {
  Fail 12 "EXTRACT_FAILED" "x07up.exe not found in archive" $x07upArchive
}

$binDir = Join-Path $Root "bin"
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
$x07upPath = Join-Path $binDir "x07up.exe"
Copy-Item -Force $found.FullName $x07upPath

Log "install toolchain via x07up"
$spec = if ($pinnedToolchain) { $Toolchain } else { $Channel }
$proc = Start-Process -FilePath $x07upPath -NoNewWindow -PassThru -Wait -ArgumentList @("--root",$Root,"--channels-url",$ChannelsUrl,"--quiet","--json","install",$spec,"--profile",$Profile,"--yes") -RedirectStandardOutput "$tmp\\x07up_install.json" -RedirectStandardError "$tmp\\x07up_install.err"
if ($proc.ExitCode -ne 0) {
  $err = Get-Content "$tmp\\x07up_install.err" -Raw -ErrorAction SilentlyContinue
  Fail 14 "TOOLCHAIN_INSTALL_FAILED" "x07up install failed" $err
}

if ($Profile -eq "full") {
  Log "install skills (user)"
  $proc = Start-Process -FilePath $x07upPath -NoNewWindow -PassThru -Wait -ArgumentList @("--root",$Root,"--channels-url",$ChannelsUrl,"--quiet","--json","skills","install","--user") -RedirectStandardOutput "$tmp\\x07up_skills.json" -RedirectStandardError "$tmp\\x07up_skills.err"
}

$x07Path = Join-Path $binDir "x07.exe"
$proc = Start-Process -FilePath $x07Path -NoNewWindow -PassThru -Wait -ArgumentList @("--help") -RedirectStandardOutput "$tmp\\x07_help.out" -RedirectStandardError "$tmp\\x07_help.err"
if ($proc.ExitCode -ne 0) {
  $err = Get-Content "$tmp\\x07_help.err" -Raw -ErrorAction SilentlyContinue
  Fail 14 "TOOLCHAIN_INSTALL_FAILED" "x07 not runnable after install" $err
}

$modifiedPath = $false
$pathHint = "`$env:PATH = `"$binDir;`$env:PATH`""
$pathWarn = ""
if (-not $NoModifyPath) {
  try {
    $currentUserPath = [Environment]::GetEnvironmentVariable("PATH","User")
    if (-not $currentUserPath) { $currentUserPath = "" }
    if ($currentUserPath -notlike "*$binDir*") {
      $newUserPath = if ($currentUserPath) { "$binDir;$currentUserPath" } else { "$binDir" }
      [Environment]::SetEnvironmentVariable("PATH", $newUserPath, "User")
      $modifiedPath = $true
    }
  } catch {
    $pathWarn = $_.Exception.Message
  }
}

$report = @{
  schema_version = "x07.install.report@0.1.0"
  ok = $true
  root = $Root
  bin_dir = $binDir
  channels_url = $ChannelsUrl
  channel = $Channel
  toolchain = $Toolchain
  x07up = $x07upTag
  profile = $Profile
  target = $target
  modified_path = $modifiedPath
  path_hint = $pathHint
  installed = @{
    x07up_path = $x07upPath
    x07_path = (Join-Path $binDir "x07.exe")
    toolchain_dir = (Join-Path $Root "toolchains\\$Toolchain")
    components = if ($Profile -eq "full") { @("docs","skills") } else { @() }
  }
  warnings = @()
}
if ($pathWarn) { $report.warnings += "path modify failed: $pathWarn" }

if ($Json) {
  $report | ConvertTo-Json -Depth 6
} else {
  Write-Host "ok: installed X07 to $Root"
  Write-Host $pathHint
  Write-Host ""
  Write-Host "next: create a project"
  Write-Host "  mkdir myapp; cd myapp; x07 init"
  Write-Host "  x07 run"
  Write-Host "  x07 test --manifest tests/tests.json"
  Write-Host ""
  Write-Host "next: create a publishable package repo"
  Write-Host "  mkdir mypkg; cd mypkg; x07 init --package"
}
