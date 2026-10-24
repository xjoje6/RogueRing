# Arranca Elden Ring con RogueRing para pruebas en segundo plano.
# - Copia de seguridad de guardados y GraphicsConfig (solo la primera vez).
# - GraphicsConfig de pruebas: ventana 1280x720 (se restaura con cerrar.ps1).
# - Monitor virtual Parsec y ventana del juego movida alli sin quitar el foco.
param([switch]$SinPantallaVirtual, [switch]$Jugar, [string]$Perfil = "", [string]$Me3 = "", [switch]$ConOverlaySteam, [string]$Savefile = "RogueRing.sl2")
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$rt = Join-Path $root "runtime"
# -Me3: otro me3.exe (p. ej. el oficial, para probar el paquete como lo usará un jugador).
$me3 = if ($Me3 -ne "") { $Me3 } else { Join-Path $root "tools\vendor\me3\target\x86_64-pc-windows-msvc\release\me3.exe" }
# Partidas del usuario de Steam con sesión abierta (SteamID64 = 76561197960265728 + id de cuenta); si
# Steam no está abierto, la única carpeta de usuario que haya.
$cuenta = (Get-ItemProperty "HKCU:\Software\Valve\Steam\ActiveProcess" -ErrorAction SilentlyContinue).ActiveUser
$er = Join-Path $env:APPDATA "EldenRing"
$saves = if ($cuenta) { Join-Path $er ([string](76561197960265728 + [int64]$cuenta)) } else { (Get-ChildItem $er -Directory | Where-Object Name -Match '^\d{17}$' | Select-Object -First 1).FullName }
$gfx = Join-Path $env:APPDATA "EldenRing\GraphicsConfig.xml"
New-Item -ItemType Directory -Force $rt | Out-Null

# 1. Copias de seguridad (nunca se sobrescriben)
$bk = Join-Path $rt "backup_saves"
if (-not (Test-Path $bk)) { Copy-Item $saves $bk -Recurse; Write-Output "copia de guardados en $bk" }
$gfxBk = Join-Path $rt "GraphicsConfig.jose.xml"
if (-not (Test-Path $gfxBk)) { Copy-Item $gfx $gfxBk; Write-Output "copia de GraphicsConfig en $gfxBk" }

# 2. GraphicsConfig de pruebas (UTF-16 como el original)
$enc = New-Object Text.UnicodeEncoding($false, $false)  # UTF-16 LE sin BOM, como el original
$x = $enc.GetString([IO.File]::ReadAllBytes($gfxBk))
$x = $x -replace "<ScreenMode>\w+</ScreenMode>", "<ScreenMode>WINDOW</ScreenMode>"
$x = $x -replace "<Resolution-WindowScreenWidth>\d+</", "<Resolution-WindowScreenWidth>1280</"
$x = $x -replace "<Resolution-WindowScreenHeight>\d+</", "<Resolution-WindowScreenHeight>720</"
# Para jugar se deja la configuración gráfica tal cual.
if (-not $Jugar) { [IO.File]::WriteAllBytes($gfx, $enc.GetBytes($x)) }

# 3. Mod (eventos generados desde extract/), DLL del banco de pruebas y perfil de me3
Push-Location (Join-Path $root "tools\ModBuilder\bin\Release\net10.0")
& .\ModBuilder.exe $root | Out-Null
$mb = $LASTEXITCODE
Pop-Location
# Si ModBuilder se para, el mod se queda a medias (regulation.bin de antes con datos nuevos).
if ($mb -ne 0) { throw "ModBuilder ha fallado (código $mb): ejecútalo a mano para ver el error" }
Copy-Item (Join-Path $root "dll\target\release\rogue_harness.dll") (Join-Path $rt "rogue_harness.dll") -Force
$profile = @"
profileVersion = "v1"
savefile = "$Savefile"

[[supports]]
game = "eldenring"

[[packages]]
path = '$root\mod'

[[natives]]
path = '$rt\rogue_harness.dll'
"@
Set-Content (Join-Path $rt "rogue-ring.me3") $profile -Encoding ASCII
Remove-Item (Join-Path $rt "harness\cmd.txt"), (Join-Path $rt "harness\out.txt") -ErrorAction SilentlyContinue

# 4. Monitor virtual
if (-not $SinPantallaVirtual) {
    $stop = Join-Path $rt "pantalla.stop"
    Start-Process (Join-Path $root "dll\target\release\pantalla.exe") -ArgumentList "run", "`"$stop`"" -WindowStyle Hidden
    Start-Sleep -Seconds 3
}

# 5. Steam: me3 no arranca sin él. Si no está abierto, se abre y se le dan unos segundos para entrar.
if (-not (Get-Process steam -ErrorAction SilentlyContinue)) {
    Start-Process (Get-ItemProperty "HKCU:\Software\Valve\Steam").SteamExe -ArgumentList "-silent"
    for ($i = 0; $i -lt 30 -and -not (Get-Process steamwebhelper -ErrorAction SilentlyContinue); $i++) { Start-Sleep -Seconds 2 }
    Start-Sleep -Seconds 15
}

# 6. Juego
# Sin overlay de Steam en este proceso: una A a destiempo en el título abría la tienda de DLC dentro del juego.
# -Savefile: otro nombre de partida (pruebas de la partida plantilla sin tocar la de verdad).
$env:ROGUE_SAVEFILE = $Savefile
# -ConOverlaySteam: como lo tiene un jugador (para probar el arranque directo con el overlay activo).
if (-not $ConOverlaySteam) { $env:SteamNoOverlayUIDrawing = "1" }
# -Perfil: lanzar otro perfil de me3 (p. ej. el del paquete de dist/, para probarlo tal cual se publica).
$perfilUsado = if ($Perfil -ne "") { $Perfil } else { "$rt\rogue-ring.me3" }
Start-Process $me3 -ArgumentList "launch", "--profile", "`"$perfilUsado`"" -WindowStyle Hidden
Write-Output "lanzado"

# 7. Pruebas: la ventana al monitor virtual ANTES del foco simulado y la partida cargada
# (tools/autocargar.py). Con el foco simulado y la ventana en la pantalla del usuario, el juego le
# devolvía el ratón al centro de su monitor en cada fotograma.
if (-not $Jugar -and -not $SinPantallaVirtual) {
    python (Join-Path $PSScriptRoot "autocargar.py")
}
