$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\release\minipos.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Compile primero con: cargo build --release -p minipos' }
$pdfinfo = Get-Command pdfinfo -ErrorAction SilentlyContinue
if (-not $pdfinfo) { throw 'Instale Poppler pdfinfo para ejecutar esta verificación externa.' }
$data = Join-Path ([System.IO.Path]::GetTempPath()) ("minipos-pdf-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $data | Out-Null
try {
    & $exe --instance PDF01 --data-dir $data --demo-sale
    if ($LASTEXITCODE -ne 0) { throw "minipos.exe terminó con código $LASTEXITCODE" }
    $pdf = Get-ChildItem -LiteralPath (Join-Path $data 'comprobantes') -Filter 'ticket.pdf' -Recurse | Select-Object -First 1
    & $pdfinfo.Source $pdf.FullName
    if ($LASTEXITCODE -ne 0) { throw 'pdfinfo no pudo abrir el PDF generado.' }
    Write-Output 'PASS: Poppler abrio el ticket y leyo la tabla de paginas.'
}
finally {
    Remove-Item -LiteralPath $data -Recurse -Force
}
