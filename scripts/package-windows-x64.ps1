$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist'
$variants = @(
    @{ Name = 'x64'; Executable = (Join-Path $root 'target\release\minipos.exe') },
    @{ Name = 'x86'; Executable = (Join-Path $root 'target\i686-pc-windows-msvc\release\minipos.exe') }
)
$hashes = @()
foreach ($variant in $variants) {
    if (-not (Test-Path -LiteralPath $variant.Executable -PathType Leaf)) {
        $targetName = if ($variant.Name -eq 'x86') { 'i686-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
        throw "Falta $($variant.Name). Compile con: cargo build --release -p minipos --target $targetName"
    }
    $stage = Join-Path $dist "MiniPOS-Windows-$($variant.Name)"
    $distRoot = [System.IO.Path]::GetFullPath($dist).TrimEnd('\')
    $stage = [System.IO.Path]::GetFullPath($stage)
    if (-not $stage.StartsWith("$distRoot\", [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "La carpeta temporal del paquete sale de dist: $stage"
    }
    if (Test-Path -LiteralPath $stage) {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $stage | Out-Null
    Copy-Item -LiteralPath $variant.Executable -Destination (Join-Path $stage 'minipos.exe') -Force
    Copy-Item -LiteralPath (Join-Path $root 'docs\PORTABLE-WINDOWS.md') -Destination (Join-Path $stage 'LEEME.md') -Force
    Copy-Item -LiteralPath (Join-Path $root 'config.demo.toml') -Destination (Join-Path $stage 'config.demo.toml') -Force
    New-Item -ItemType Directory -Force -Path (Join-Path $stage 'scripts') | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'start-three-boxes.ps1') -Destination (Join-Path $stage 'scripts') -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'smoke-three-instances.ps1') -Destination (Join-Path $stage 'scripts') -Force
    $zip = Join-Path $dist "MiniPOS-Windows-$($variant.Name)-0.1.0.zip"
    if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
    $hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    $hashes += "$hash  $(Split-Path -Leaf $zip)"
    Write-Output "Paquete: $zip"
    Write-Output "SHA-256: $hash"
}
Set-Content -LiteralPath (Join-Path $dist 'SHA256SUMS.txt') -Value $hashes -Encoding ascii
