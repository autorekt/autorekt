# Installs autorekt on Windows: irm {{SITE}}/install.ps1 | iex
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$base = '{{SITE}}/dl'
$arch = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
  'X64' { 'x64' }
  'Arm64' { 'arm64' }
  default { throw "autorekt: no build for $_" }
}
$file = "autorekt-windows-$arch.exe"

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
  Write-Host "downloading $base/$file"
  Invoke-WebRequest -UseBasicParsing "$base/$file" -OutFile "$tmp\$file"
  Invoke-WebRequest -UseBasicParsing "$base/SHA256SUMS" -OutFile "$tmp\SHA256SUMS"
  $want = (Get-Content "$tmp\SHA256SUMS" | Where-Object { $_ -match " $([regex]::Escape($file))$" }) -replace ' .*', ''
  $got = (Get-FileHash -Algorithm SHA256 "$tmp\$file").Hash.ToLower()
  if (-not $want -or $want -ne $got) { throw "autorekt: checksum mismatch for $file" }

  $dir = Join-Path $env:USERPROFILE '.local\bin'
  $exe = Join-Path $dir 'autorekt.exe'
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  # Windows cannot replace a running program, so a running daemon is stopped first.
  if (Test-Path $exe) { & $exe daemon stop | Out-Null }
  Move-Item -Force "$tmp\$file" $exe
  Write-Host "installed $(& $exe --version) to $exe"
} finally {
  Remove-Item -Recurse -Force $tmp
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (($userPath -split ';') -contains $dir)) {
  [Environment]::SetEnvironmentVariable('Path', (@($userPath, $dir) | Where-Object { $_ }) -join ';', 'User')
  $env:Path = "$env:Path;$dir"
  Write-Host "added $dir to your PATH; open a new terminal to use it there"
}
Write-Host 'next: autorekt show'
