# Cierra el juego, quita el monitor virtual y restaura el GraphicsConfig de antes de las pruebas.
param([switch]$Forzar)
$root = Split-Path -Parent $PSScriptRoot
$rt = Join-Path $root "runtime"
# El juego abierto puede ser la partida del jugador (su carpeta de juego escribe su propio registro): si
# su registro se ha escrito hace poco y el del banco no, no se cierra (-Forzar para cerrarlo igual).
$juego = Get-Process eldenring -ErrorAction SilentlyContinue
if ($juego -and -not $Forzar) {
    $suyo = Join-Path ([Environment]::GetFolderPath("Desktop")) "RogueRing\runtime\harness\harness.log"
    $banco = Join-Path $rt "harness\harness.log"
    $reciente = { param($p) (Test-Path $p) -and ((Get-Date) - (Get-Item $p).LastWriteTime).TotalSeconds -lt 120 }
    if ((& $reciente $suyo) -and -not (& $reciente $banco)) {
        throw "Elden Ring abierto con la partida del jugador (su registro es reciente y el del banco no): no se cierra"
    }
}
$juego | Stop-Process -Force
Start-Sleep -Seconds 2
New-Item -ItemType File -Force (Join-Path $rt "pantalla.stop") | Out-Null
$gfxBk = Join-Path $rt "GraphicsConfig.jose.xml"
if (Test-Path $gfxBk) { Copy-Item $gfxBk (Join-Path $env:APPDATA "EldenRing\GraphicsConfig.xml") -Force; Write-Output "GraphicsConfig restaurado" }
