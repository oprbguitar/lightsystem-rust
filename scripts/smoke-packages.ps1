$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist'
foreach ($arch in @('x64', 'x86')) {
    $zip = Join-Path $dist "MiniPOS-Windows-$arch-0.1.0.zip"
    $work = Join-Path ([System.IO.Path]::GetTempPath()) ("minipos-package-" + $arch + '-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        Expand-Archive -LiteralPath $zip -DestinationPath $work
        $package = $work
        $exe = Join-Path $package 'minipos.exe'
        $data = Join-Path $work 'datos'
        New-Item -ItemType Directory -Path $data | Out-Null
        & $exe --instance "ZIP$arch" --data-dir $data --demo-sale
        if ($LASTEXITCODE -ne 0) { throw "El paquete $arch terminó con código $LASTEXITCODE" }
        $ticket = Get-ChildItem -LiteralPath (Join-Path $data 'comprobantes') -Filter 'ticket.pdf' -Recurse | Select-Object -First 1
        if (-not $ticket) { throw "El paquete $arch no generó ticket.pdf" }
        Write-Output "PASS: paquete $arch se ejecuto desde su carpeta extraida."
    }
    finally {
        Remove-Item -LiteralPath $work -Recurse -Force
    }
}
