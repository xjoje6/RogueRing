# Para jugar tú: arranca en tu pantalla (sin monitor virtual) como lo verá un jugador: el mod tapa el
# título, carga solo la partida y abre su menú principal. Usa tu configuración gráfica normal.
# -Semilla XXXXXX: salta el menú y empieza directamente una run con esa semilla (pruebas).
# -Savefile: otra partida del mod (la rama de pruebas usa RogueRingRama.sl2; si no existe, se crea la plantilla).
param([string]$Semilla = "", [string]$Savefile = "RogueRing.sl2")
$t = $PSScriptRoot
& "$t\cerrar.ps1" | Out-Null
Start-Sleep -Seconds 3
& "$t\lanzar.ps1" -SinPantallaVirtual -Jugar -ConOverlaySteam -Savefile $Savefile | Out-Null
if ($Semilla -ne "") { python (Join-Path $t "jugar.py") $Semilla }
