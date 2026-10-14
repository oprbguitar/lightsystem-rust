$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\release\minipos.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { $exe = Join-Path $root 'minipos.exe' }
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Compile primero con: cargo build --release -p minipos' }
$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("minipos-smoke-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $temp | Out-Null
try {
    $procs = @()
    foreach ($instance in @('CAJA01', 'CAJA02', 'CAJA03')) {
        $dataDir = Join-Path $temp $instance.ToLowerInvariant()
        New-Item -ItemType Directory -Path $dataDir | Out-Null
        $arguments = @('--instance', $instance, '--data-dir', $dataDir, '--demo-sale')
        $procs += Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $root -PassThru -Wait:$false
    }
    foreach ($process in $procs) {
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) { throw "minipos.exe terminó con código $($process.ExitCode)" }
    }
    foreach ($instance in @('CAJA01', 'CAJA02', 'CAJA03')) {
        $dataDir = Join-Path $temp $instance.ToLowerInvariant()
        $db = Join-Path $dataDir 'minipos.sqlite3'
        $tickets = @(Get-ChildItem -LiteralPath (Join-Path $dataDir 'comprobantes') -Directory)
        if (-not (Test-Path -LiteralPath $db) -or $tickets.Count -ne 1 -or -not (Test-Path (Join-Path $tickets[0].FullName 'ticket.txt')) -or -not (Test-Path (Join-Path $tickets[0].FullName 'ticket.pdf'))) {
            throw "Falló la verificación de $instance en $dataDir"
        }
        if (-not (Test-Path (Join-Path $tickets[0].FullName 'ticket.escpos')) -or -not (Test-Path (Join-Path $tickets[0].FullName 'metadata.json'))) { throw "Faltan archivos del ticket de $instance" }
    }
    Write-Output 'PASS: tres procesos, tres bases SQLite y tres pares de tickets TXT/PDF independientes.'
}
finally {
    Remove-Item -LiteralPath $temp -Recurse -Force
}
