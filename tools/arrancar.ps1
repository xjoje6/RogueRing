# Ciclo completo sin intervención: cerrar lo que haya, lanzar, mover al monitor virtual y cargar el perfil.
$t = $PSScriptRoot
& "$t\cerrar.ps1" | Out-Null
Start-Sleep -Seconds 4
& "$t\lanzar.ps1" | Out-Null
python (Join-Path $t "autocargar.py")
