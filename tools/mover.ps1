# Mueve la ventana del juego al monitor virtual (sin quitar el foco a nadie).
$root = Split-Path -Parent $PSScriptRoot
$pos = Get-Content (Join-Path $root "runtime\pantalla.pos") -ErrorAction Stop
$x, $y, $w, $h = $pos -split " "
python (Join-Path $PSScriptRoot "banco.py") ventana ([int]$x + 40) ([int]$y + 40) 1296 759
