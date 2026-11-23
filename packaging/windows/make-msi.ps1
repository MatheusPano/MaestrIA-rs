$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..\..')

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$rev = Select-String -Path crates\mx-app\Cargo.toml -Pattern '^revision\s*=\s*"([^"]+)"' | Select-Object -First 1
$revision = if ($rev) { $rev.Matches[0].Groups[1].Value } else { '1' }
if (-not $env:MAESTRIA_VERSION) { $env:MAESTRIA_VERSION = $version }

cargo build --release -p mx-app --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
  dotnet tool install --global wix --version 5.0.2
  $env:PATH += ";$env:USERPROFILE\.dotnet\tools"
}

$dist = 'packaging\windows\dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$out = "$dist\maestria-rs_$version-${revision}_x64.msi"
wix build packaging\windows\maestria-rs.wxs -arch x64 `
  -d Version=$version `
  -d ExePath=target\release\maestria.exe `
  -d IconPath=packaging\windows\maestria-rs.ico `
  -o $out
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "pronto: $out"
