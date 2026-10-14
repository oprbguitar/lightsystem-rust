$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $projectRoot 'target\release\minipos.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { $exe = Join-Path $projectRoot 'minipos.exe' }
$exe = [System.IO.Path]::GetFullPath($exe)
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    throw "No existe $exe. Compile primero con: cargo build --release -p minipos"
}
$root = Join-Path $projectRoot 'datos'
foreach ($instance in @('CAJA01', 'CAJA02', 'CAJA03')) {
    $dataDir = Join-Path $root $instance.ToLowerInvariant()
    New-Item -ItemType Directory -Force -Path $dataDir | Out-Null
    $args = "--instance $instance --data-dir `"$dataDir`" --demo"
    Start-Process -FilePath $exe -ArgumentList $args -WorkingDirectory (Split-Path -Parent $exe) -PassThru | Out-Null
}
