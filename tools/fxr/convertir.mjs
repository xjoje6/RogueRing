// FXR de Nightreign a Elden Ring con @cccode/fxr (EvenTorset): node convertir.mjs <entrada> <salida>
// Convierte cada .fxr de la carpeta de entrada y lo deja con el mismo nombre en la de salida. La
// conversión a mano de los bytes (recortar campos) dejaba cosas que ER no tiene, como las curvas
// ComponentHermite de NR, y el juego cargaba los efectos sin dibujarlos (Fundidor sin fuego, 18-10).
// Una línea por archivo que falla («fallo <nombre>: <motivo>»); los demás no se tocan.
import { FXR, Game } from '@cccode/fxr'
import fs from 'node:fs'
import path from 'node:path'

const [entrada, salida] = process.argv.slice(2)
fs.mkdirSync(salida, { recursive: true })
let bien = 0, mal = 0
for (const f of fs.readdirSync(entrada).filter(f => f.endsWith('.fxr'))) {
  try {
    const fx = FXR.read(fs.readFileSync(path.join(entrada, f)), Game.Nightreign)
    fs.writeFileSync(path.join(salida, f), Buffer.from(fx.toArrayBuffer(Game.EldenRing)))
    bien++
  } catch (e) {
    console.log(`fallo ${f}: ${e.message}`)
    mal++
  }
}
console.log(`convertidos ${bien}, fallidos ${mal}`)
