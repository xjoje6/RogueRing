# Prepara Seamless Co-op para RogueRing antes de abrir el juego (lo llama «Play RogueRing (Seamless).bat»;
# en el paquete va en runtime\seamless.ps1). Así nadie tiene que tocar el ersc_settings.ini a mano:
# - la contraseña de la sesión: si no hay, se pide una vez (la misma para todos) y se guarda;
# - partida aparte (save_file_extension = rogue): con co2 sería la partida de Seamless del jugador;
# - sin invasores ni castigo por morir;
# - escalado de Seamless a cero: el del cooperativo lo pone el mod (data/escalado.json, coop_extra).
# Nunca enseña la contraseña.
$ErrorActionPreference = "Stop"
$raiz = Split-Path -Parent $PSScriptRoot
$carpeta = Join-Path $raiz "SeamlessCoop"
$ini = Join-Path $carpeta "ersc_settings.ini"
if (-not (Test-Path (Join-Path $carpeta "ersc.dll"))) {
    Write-Host "Falta Seamless Co-op: pon su carpeta SeamlessCoop junto a este lanzador."
    Write-Host "Seamless Co-op is missing: put its SeamlessCoop folder next to this launcher."
    exit 1
}
$script:texto = if (Test-Path $ini) { [IO.File]::ReadAllText($ini) } else { "" }

function Valor([string]$clave) {
    $m = [regex]::Match($script:texto, "(?m)^\s*$clave\s*=[ \t]*(.*?)\s*$")
    if ($m.Success) { $m.Groups[1].Value } else { "" }
}

function Poner([string]$clave, [string]$valor) {
    $re = [regex]"(?m)^(\s*$clave\s*=)[^\r\n]*"
    if ($re.IsMatch($script:texto)) {
        $script:texto = $re.Replace($script:texto, { param($m) $m.Groups[1].Value + " " + $valor }, 1)
    } else {
        $script:texto = $script:texto.TrimEnd() + "`r`n$clave = $valor`r`n"
    }
}

if ((Valor "cooppassword") -eq "") {
    Write-Host ""
    Write-Host "Contraseña de la sesión: la misma para todos los que vais a jugar juntos."
    Write-Host "Session password: the same for everyone playing together."
    do { $clave = (Read-Host "Contraseña / Password").Trim() } while ($clave -eq "")
    Poner "cooppassword" $clave
}
Poner "save_file_extension" "rogue"
# Al morir, mirar a los compañeros hasta que caigan todos (docs/COOP_PROTOCOLO.md).
Poner "always_spectate_on_death" "0"
foreach ($k in "allow_invaders", "death_debuffs", "enemy_health_scaling", "enemy_damage_scaling", "enemy_posture_scaling",
               "boss_health_scaling", "boss_damage_scaling", "boss_posture_scaling") {
    Poner $k "0"
}
[IO.File]::WriteAllText($ini, $script:texto)
exit 0
