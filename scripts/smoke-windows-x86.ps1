$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\i686-pc-windows-msvc\release\minipos.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Compile primero con: cargo build --release -p minipos --target i686-pc-windows-msvc' }
$data = Join-Path ([System.IO.Path]::GetTempPath()) ("minipos-x86-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $data | Out-Null
try {
    & $exe --instance CAJAX86 --data-dir $data --demo-sale
    if ($LASTEXITCODE -ne 0) { throw "minipos x86 terminó con código $LASTEXITCODE" }
    $tickets = @(Get-ChildItem -LiteralPath (Join-Path $data 'comprobantes') -Directory)
    if ($tickets.Count -ne 1 -or -not (Test-Path (Join-Path $tickets[0].FullName 'ticket.pdf'))) { throw 'El ejecutable x86 no generó su ticket PDF.' }
    Write-Output 'PASS: proceso x86 completo una venta demo y genero el PDF bajo WoW64.'
}
finally {
    Remove-Item -LiteralPath $data -Recurse -Force
}
