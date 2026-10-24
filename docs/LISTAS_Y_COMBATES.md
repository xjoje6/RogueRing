# Listas por acto, combates variados y cooperativo

Estudio del 15-10-2026. La propuesta completa, con ids reales (modelo, NpcParam e IA), está en
`docs/borradores/listas_propuesta.json`. El juego no la usa todavía.

Marcas:
- **validado**: probado en el coliseo en septiembre; aparece, se mueve y muere bien. Los jefes del
  acto 1 y 2 están además comprobándose ahora con el jefe colocado en el MSB.
- **probar**: nuevo; hay que validarlo antes de meterlo en una run.

## Hecho en la rama `pruebas/contenido` (15-10)

- **Datos.**
  - `data/acts/1-3.json` con las listas de la propuesta: 99, 107 y 55 encuentros, más los jefes de
    Nightreign (docs/JEFES_Y_MOVESETS_DE_OTROS_JUEGOS.md).
  - `data/coop/1-3.json` con los 56 dúos.
  - El acto 3 se llama «La Tierra Sombría» y usa los tramos 7180-7210.
- **Formatos.** Todos los de este documento: manada, escolta, dúo, oleadas, racha (doble recompensa),
  aguante (`segundos`), contrarreloj (`segundos` en un combate normal: una bendición más si se gana a
  tiempo) y campeón (`mutadores` del encuentro).
- **Cooperativo.** Una run de sesión usa las listas de dúos y los combates de grupo de cada acto.
- **Validación** (`tools/validar_msb.py`, todos los encuentros, colocados en el MSB):
  - 305 de 317 salen, se mueven y se ganan a la primera.
  - De los 12 que se quedaron quietos en 2,5 s, 9 se mueven con el jugador cerca (`--cerca`, a 6 m):
    esperaban a verlo (Oso rojo, Vyke, Bailarina de Ranah, Caballero de la muerte...).
  - Fuera de las listas: las dos Vírgenes raptoras (16000860 y n2_c5970) y el Campeón
    de Fia (12030800, también su dúo con Esgar), que no se mueven nunca.
  - «Aguanta al Avatar» queda por mirar: el Avatar suelto sí se mueve (se entierra y sale lejos).

## En corto

| | Ahora | Propuesta | De uno en uno | Grupos (manada, escolta, dúo, oleadas) |
|---|---|---|---|---|
| Acto 1 | 65 | 96 | 76 | 20 |
| Acto 2 | 70 | 104 | 85 | 19 |
| Acto 3 | no hay | 51 | 46 | 5 |
| Total | 135 | 251 | 207 | 44 |

- **Acto 1:** el más lleno de enemigos normales. Hay 9 nuevos de uno en uno, 16 manadas, 3 escoltas y
  una horda.
- **Acto 2:** el juego base de la mitad hacia arriba y el DLC medio: 15 jefes de campo del DLC, 20
  normales nuevos y 16 manadas.
- **Acto 3:** lo más duro, con 12 señores de primera (Malenia, Radahn consorte, Gaius, Midra, Bayle,
  Maliketh, Godfrey/Hoarah Loux, Metyr, Mohg, la Bestia de Elden, Placidusax y Messmer).
- **Cooperativo:** 56 dúos en listas aparte (22, 19 y 15 por acto), con dos jefes donde se puede.
- De los 251 encuentros, 168 ya están validados; los 83 nuevos hay que probarlos.

## De dónde sale

- **Jefes:** el catálogo de septiembre tiene 212.
  - En la validación del coliseo, 186 aparecían, se movían y morían bien. Los 16 que se quedaban
    quietos (Dúo sacrodermo, Astel nacida del vacío, Fortissax, Gideon, Jori, los caracoles...) quedan
    fuera.
  - Fuera por decisión: dragones voladores (Agheel, Adula, Smarag, Lansseax, Greyll), Rykard, los
    cristalianos, Mímico, Tibia y Parches.
  - Hoy en los actos hay 125. La propuesta mete unos 40 más, casi todos del DLC, que estaban validados
    pero sin usar.
- **Enemigos normales:**
  - Hay unos 330 modelos (186 del juego base y 150 del DLC) con fila base de NpcParam e IA
    (`cXXXX0000`). Así se hicieron los 10 `coliseo_*` del acto 1.
  - Los nombres salen de los alias de Smithbox; el español está puesto a mano.
- **Jefes sin datos en el catálogo:** Messmer (c5130), Radahn de Caelid (c4730) y el Hipopótamo dorado
  (c5010) van con su fila base. Hay que probarlos.

## Criterios

- **Acto 1, Limgrave y alrededores:**
  - Muchos enemigos normales: de uno en uno al principio, en manadas después y los jefes de campo
    flojos como normales.
  - Señores: Margit, Godrick, Rennala, el Lobo rojo de Radagon, Loretta (Caria) y el Espíritu
    ancestral majestuoso. Bajan del acto 2 Margit, el Lobo rojo y Loretta. Suben Mohg el Augurio y
    Godfrey (sombra).
- **Acto 2, las tierras altas y el DLC medio:**
  - Las normales de ahora, más caballeros de Leyndell, Melena Roja, mausoleo y Árbol Hierático, y
    los normales del DLC (cornudos, herreros gólem, inquisidores, soldados de Messmer).
  - Élites: las del acto 2 por debajo de 60 000 runas y 15 jefes de campo del DLC (Onze, Rakshasa,
    Edredd, Bailarina de Ranah, Caballeros de la Muerte, Centinelas, Labirith...).
  - Señores: Morgott, Mohg el Augurio, Godfrey (sombra), Loretta del Árbol Hierático, el León
    Danzante, el Dúo sacrodermo rehecho con Apóstol y Noble, y Radahn de Caelid (probar).
- **Acto 3, lo más duro:**
  - Normales: las élites del acto 2 de 60 000 runas o más (Fieles de la Hoja Negra, Niall, Alecto,
    Vyke, Astel, Theodorix...) y los caballeros del DLC: de fuego, negros, hojamalditas y la escuadra
    de Messmer.
  - Élites: los jefes de campo más duros del DLC (Grandes osos rojos, Caballero putrefacto, Conde
    Ymir, Avatar del Árbol Umbrío, Bestia de la Estrella fugaz), más Rellana y Romina.
  - Señores: los de primera. Bayle y Placidusax están validados, pero son dragones (vuelan y se
    teletransportan): probarlos en serio antes de dejarlos.

## Las listas

### Acto 1

**Débiles (41)**

- Validados (16): Perfumista Tricia; Soldado de Godrick; Hombre bestia de Farum Azula; Jefe semihumano; Trol excavapiedra; Sombra de cementerio (×2); Caballero desterrado; Soldado exiliado grande; Trol; Cangrejo gigante; Caballero de Godrick; Monje de fuego; Hombre serpiente; Mercenario de Kaiden; Heraldo oráculo grande.
- Nuevos, por probar (9): Caballero cuco; Perro gigante; Lobo del arquero albináurico; Hombre de arcilla; Mago de batalla; Tinaja viviente guerrera; Rata gigante; Pulpo de tierra grande; Semihumano grande.
- Manada: Patrulla de Godrick (4 enemigos).
- Manada: Semihumanos (5 enemigos).
- Manada: Manada de lobos (4 enemigos).
- Manada: Exiliados (4 enemigos).
- Manada: Esqueletos de la catacumba (4 enemigos).
- Manada: Bastardos (3 enemigos).
- Manada: Salteadores (4 enemigos).
- Manada: Guardia de Raya Lucaria (4 enemigos).
- Manada: Jarras vivientes (4 enemigos).
- Manada: Nido de ratas (5 enemigos).
- Manada: Perros callejeros (4 enemigos).
- Manada: Hechiceros de la academia (3 enemigos).
- Manada: Seguidores ancestrales (4 enemigos).
- Manada: Hombres de arcilla (3 enemigos).
- Manada: Cangrejos (5 enemigos).
- Manada: Basiliscos (3 enemigos).

**Normales (27)**

- Validados (24): Lunático calabezudo; Perro guardián funerario del Árbol Áureo (×3); Asesina de los Cuchillos Negros (×2); Duelista guardián de las tumbas; Bastardo escamoso; Caballería nocturna (×4); Oso rúnico; Ave de la muerte; Renacido regio; Vástago injertado; Caballero Imputrescible; Señor de ónice; Caballero sabueso; Bastardo leonino; Bols, caballero cariano; Asesino de augurios; Viejo héroe de Zamor; Cara de gusano gigante.
- Nuevos, por probar (1): Trol caballero cariano.
- Escolta: Caballero de Godrick y su patrulla (3 enemigos).
- Escolta: Jefe semihumano y su tribu (4 enemigos).

**Élites (22)**

- Validados (20): Caballero del crisol (×2); Gólem guardián; Cazador de esferas (×2); Centinela agreste; Avatar del Árbol Áureo (×2); Monje de Nox; Ave de la muerte; Duelista frenético; Sombra de cementerio; Caballero Imputrescible lancero; Perro guardián funerario espadado del Árbol Áureo; Bestia de la Estrella fugaz; Ave del ritual fúnebre; Gilika, la reina semihumana; Caballería nocturna; Noble sanguino; Asesina de los Cuchillos Negros.
- Escolta: Reina Gilika y su corte (4 enemigos).
- Oleadas: Horda del coliseo (9 enemigos).

**Señores del acto (6)**

- Validados (6): Godrick, el Injertado; Rennala, Reina de la Luna Llena; Espíritu ancestral majestuoso; Margit, el Augurio Caído; Lobo Rojo de Radagon; Guardia Real Loretta.

### Acto 2

**Normales (66)**

- Validados (30): Danzarín cristaliano; Guerrero bastardo; Avatar putrefacto; Trol excavapiedra; Caballería nocturna; Virgen raptora con guadañas pendulares; Ancestro de la putrefacción; Asesina de los Cuchillos Negros; Bestia de la Estrella fugaz; Margot, la reina semihumana; Maggie, la reina semihumana; Espíritu ancestral; Apóstol sacrodermo; Ave del ritual fúnebre; Espíritu del árbol ulcerado (×2); Duelista guardián de las tumbas; Caballero del crisol; Soldado draconiano; Señor de ónice; Dragón de magma; Centinela agreste; Cazador de esferas; Viejo héroe de Zamor; Bestia adulta de la Estrella fugaz; Lobo Rojo del Campeón; Elemer de la Zarza; Dragón de magma Makar; Caballera del crisol Siluria; Godefroy, el Injertado.
- Nuevos, por probar (20): Caballero de Leyndell; Caballero Melena Roja; Caballero del mausoleo; Caballero del Árbol Hierático; Prelado de fuego; Monje de la llama negra; Trol de las nieves; Esqueleto gigante; León viejo; Caragusano; Heraldo oráculo gigante; Cuervo gigante; Guerrero cornudo; Herrero gólem; Inquisidor gordo; Oso rojo; Bastardo leonino; Asesino de augurios; Virgen raptora; Entrañas de tinaja.
- Manada: Caballeros de Leyndell (4 enemigos).
- Manada: Melenas rojas (4 enemigos).
- Manada: Guardia del mausoleo (3 enemigos).
- Manada: Guardia del Árbol Hierático (3 enemigos).
- Manada: Monjes de fuego (3 enemigos).
- Manada: Guardianes del Árbol Áureo (4 enemigos).
- Manada: Parientes de la podredumbre (3 enemigos).
- Manada: Marionetas de Gelmir (4 enemigos).
- Manada: Hombres serpiente (3 enemigos).
- Manada: Hombres bestia de Farum Azula (3 enemigos).
- Manada: Albináuricos (4 enemigos).
- Manada: Guerreros con cuernos (3 enemigos).
- Manada: Soldados de Messmer (4 enemigos).
- Manada: Sanguinarios (3 enemigos).
- Manada: Herreros gólem (2 enemigos).
- Manada: Inquisidores (4 enemigos).

**Élites (31)**

- Validados (28): Caballero del crisol Ordovis; Ave de la muerte; Gemelo Caído; Gárgola denodada; Esgar, Sacerdote de la Sangre; Caballería nocturna (×2); Ekzykes, el putrefacto; Campeón de Fia; Espíritu del árbol ulcerado; Cazador de esferas; Noble sacrodermo; Soldado draconiano; Caballero de la prisión solitaria; Onze, maestro espadachín semihumano; Sanguinemigo jefe; Bailarina de Ranah; Oso Rojo; Caballero Negro Edredd; Rakshasa; Hojamaldita Labirith; Marigga, la reina semihumana; Caballero de la Muerte (×2); Centinela agreste (×2); Hombre dragón antiguo; Doliente.
- Nuevos, por probar (1): Hipopótamo dorado.
- Escolta: Jefe sanguinario y los suyos (3 enemigos).
- Oleadas: Asedio de Leyndell (7 enemigos).

**Señores del acto (7)**

- Validados (5): Loretta, centinela del Árbol Hierático; Mohg, el Augurio; Godfrey, primer Señor del Círculo; Morgott, el Rey Augurio; Bestia divina León Danzante.
- Nuevos, por probar (1): Radahn, el Azote de las Estrellas.
- Duo: Dúo sacrodermo (2 enemigos). Apóstol y Noble por separado (los dos validados).

### Acto 3

**Normales (28)**

- Validados (18): Fiel de la Hoja Negra (×2); Espíritu del árbol putrefacto; Hombre bestia de Farum Azula con machete; Avatar del Árbol Áureo; Caballero Vyke de la Mesa Redonda; Ave del ritual fúnebre (×2); Duelista guardián de las tumbas putrefacto; Alecto, líder de los Cuchillos Negros; Viejo héroe de Zamor; Comandante Niall; Avatar putrefacto (×2); Paladín bastardo; Apóstol sacrodermo; Astel, el astro fosco; Gran dragón de magma Theodorix.
- Nuevos, por probar (6): Caballero de fuego; Caballero negro; Hojamaldita; Gólem guardián; Espíritu del árbol ulcerado; Oso rúnico.
- Manada: Caballeros de fuego (3 enemigos).
- Manada: Caballeros negros (2 enemigos).
- Manada: Hojas malditas (2 enemigos).
- Manada: Escuadra de Messmer (4 enemigos).

**Élites (11)**

- Validados (10): Bestia de la Estrella fugaz; Ralva, el Gran Oso Rojo; Rugalea, el Gran Oso Rojo; Conde Ymir, madre de Dedos; Caballero putrefacto; Ave del ritual fúnebre; Avatar del Árbol Umbrío; Bestia divina León Danzante; Rellana, Caballera de Luna Gemela; Romina, la santa del brote.
- Oleadas: Horda de la Tierra Sombría (7 enemigos).

**Señores del acto (12)**

- Validados (11): Malenia, la Espada de Miquella; Radahn, consorte de Miquella; Comandante Gaius; Midra, Señor de la Llama Frenética; Metyr, madre de Dedos; Bayle el Temible; Maliketh, la Hoja Negra; Godfrey, primer Señor del Círculo; Mohg, Señor de la Sangre; Bestia del Círculo; Señor dragón Placidusax.
- Nuevos, por probar (1): Messmer el Empalador.

### Cooperativo (dúos)

**Acto 1**

- Normales (12): Asesinas de los Cuchillos Negros; Perros guardianes funerarios; Caballería nocturna; Sombras del cementerio; Bastardos; Caballero sabueso y asesina; Lunáticos calabezudos; Duelista guardián y Caballero Imputrescible; Perfumista Tricia y guerrero bastardo; Jefes semihumanos; Caballeros desterrados; Trols.
- Élites (7): Caballeros del crisol; Monjes de Nox; Duelista frenético y noble sanguino; Caballeros Imputrescibles; Aves de la muerte; Avatar y perro guardián; Centinela agreste y cazador de esferas.
- Señores del acto (3): Castillo de Velo Tormentoso: Margit y Godrick; Academia: Rennala y el Lobo rojo de Radagon; Caria: Loretta y el Espíritu ancestral majestuoso.

**Acto 2**

- Normales (6): Guerrero bastardo y caballero del crisol; Reinas semihumanas; Podredumbre: avatar y espíritu; Duelistas guardianes de las tumbas; Caballero sabueso y Elemer; Viejos héroes de Zamor.
- Élites (9): Gárgolas denodadas; Gemelos caídos; Ordovis y su compañero; Caballería nocturna del Altiplano; Esgar y el Campeón de Fia; Caballeros de la muerte; Centinelas del DLC; Onze y Marigga, la reina semihumana; Caballero negro Edredd y el Oso rojo.
- Señores del acto (4): Hermanos augurio: Morgott y Mohg; Leyndell: Godfrey (sombra) y Morgott; Dúo sacrodermo: Apóstol y Noble; El León Danzante y Loretta.

**Acto 3**

- Normales (4): Fieles de la Hoja Negra; Espíritu y avatar putrefactos; Alecto y Vyke; Niall y el paladín bastardo.
- Élites (4): Grandes osos rojos; Caballero putrefacto y ave del ritual fúnebre; Bestias de la Estrella fugaz; El Conde Ymir y el Doliente.
- Señores del acto (7): Hermanos de Miquella: Malenia y Radahn; Hermanas de la luna: Rennala y Rellana; Linaje de Godfrey: Hoarah Loux y Morgott; El general y su señor: Gaius y Radahn; Sangre y llamas: Mohg y Midra; Hojas del destino: Maliketh y Romina; Madres: Metyr y la Condesa Ymir.

## Más allá del uno contra uno

Todo sale de lo que ya funciona: un coliseo por combate. Una variante del MSB puede llevar varios
enemigos colocados, apagados hasta que la DLL enciende su flag. Eso da para todos estos formatos sin
nada nuevo en el juego:

1. **Manada.** De 2 a 5 normales del mismo sitio (patrulla de Godrick, semihumanos, lobos).
   - Se gana cuando caen todos.
   - Cada uno lleva su tramo de escalado, con la vida algo más baja, para que el grupo dure como un
     normal.
   - Una sola barra con la suma y cuántos quedan: «Patrulla de Godrick · 3».
   - Para débiles y normales.
2. **Escolta.** Un rival fuerte con 2 o 3 normales: el Caballero de Godrick con su patrulla, Gilika
   con su corte.
   - Se gana al tumbar al líder; la escolta cae con él (el EMEVD tiene «Force Character Death»).
   - La barra es la del líder.
3. **Dúo.** Dos fuertes a la vez, como las Gárgolas denodadas.
   - En solitario, cada uno con el 60 % de su vida; en cooperativo, entera.
   - Dos barras, como en el juego.
4. **Oleadas (horda).** Tres oleadas: una manada, otra mayor y un capitán.
   - Entre oleadas, 3 s y un letrero «Oleada 2».
   - Runas por oleada y la recompensa de élite al acabar.
   - Variante para el «?», **horda sin fin**: cada oleada da más runas y desde la pausa te puedes
     retirar entre oleadas con lo ganado.
5. **Racha.** Dos o tres rivales seguidos, sin recargar los viales entre medias.
   - Nodo nuevo en el mapa, una élite con corona. Recompensa doble.
6. **Campeón.** Un normal con aura: un SpEffect visual y un mutador de los que ya hay (vida ×3, más
   daño, más resistencias).
   - Convierte cualquier normal en élite, así que multiplica la variedad sin enemigos nuevos.
7. **Invasión.** Los PNJ que pelean como jugadores (Vyke, Esgar, Bailarina de Ranah, Rakshasa, Campeón
   de Fia, Conde Ymir...) con el letrero rojo de invasión.
   - Hoy salen como uno más; agrupados como «Invasor» se notan distintos.
   - Hay más invasores en los MSB del juego para ampliarlo.
8. **Contrarreloj.** Reto opcional al entrar: «Mátalo en 60 s: una bendición más». Sin castigo si no
   llegas.
9. **Aguante.** Sobrevive 60 o 90 s contra algo que no se puede matar, como una Caballería nocturna
   inmortal. Va bien para el «?».
10. **Otros coliseos.** Los de Caelid (`m45_01`) y Leyndell (`m45_02`) admiten la misma técnica.
    Dan variedad visual, y quizá espacio para los gigantes. Hay que medirlo.

Orden recomendado:
- **Primero:** manada, escolta, dúo, campeón y oleadas. Salen casi solos de las variantes.
- **Después:** racha, contrarreloj y aguante, que necesitan reglas nuevas en el director.
- **Al final:** otros coliseos.

## Cooperativo: dos jefes donde se pueda

Reglas:
- **Dos rivales siempre que se pueda**, del mismo tamaño y ritmo.
  - Nunca dos gigantes (el coliseo es pequeño) ni dos que llenen la arena de magia.
  - Mejor con historia: hermanos augurio, hermanas de la luna, linaje de Godfrey.
- **Vida.** Seamless sube un 100 % la vida de los jefes por cada jugador de más (`boss_health_scaling = 100`).
  - Con dos jefes y dos jugadores, eso sería el doble de la cuenta.
  - Recomendación: `boss_health_scaling = 0` y `enemy_health_scaling = 0` en el INI de la carpeta del
    mod, y que el escalado lo pongamos nosotros: cada jefe del dúo con su vida de solo, y +50 % por
    cada jugador a partir del tercero.
- **Débiles:** en cooperativo no hay débiles de uno; pasan a manadas, con un 50 % más de enemigos por
  jugador.
- **Manadas y oleadas:** un 50 % más de enemigos por jugador de más.
- **Señores:** dúos de leyenda; un señor sin pareja va con escolta.
- Las recompensas siguen siendo de cada uno.

Las listas de dúos están arriba, en «Cooperativo». Irían en ficheros aparte (`data/acts/coop_1.json`...)
y el director los usa cuando hay sesión de Seamless con compañeros. La semilla compartida hace que
todos saquen el mismo dúo.

## Cómo se implementa

1. **Datos.**
   - Un encuentro puede llevar `enemigos` (lista de chr, npc y think), `formato` (`solo`, `manada`,
     `escolta`, `duo`, `oleadas`), `oleadas` (índices por oleada) y `lider`.
   - Los de ahora siguen igual (un enemigo con chr, npc y think).
2. **ModBuilder.**
   - Hasta 10 huecos por variante (entidades 45000990-45000999), en arco delante del jugador. Los
     grandes, más lejos.
   - Cada oleada con su EntityGroupID (45000900 + oleada).
   - En el evento del mapa, un evento por oleada: apaga el grupo, espera su flag (1049300110 +
     oleada) y lo enciende.
   - **Probar** que «Change Character Enable State» acepta el id de grupo, como en los mapas del juego.
3. **Director.**
   - Lleva una lista de entidades. El combate está listo cuando existe la oleada 0.
   - Con una oleada vacía, enciende la siguiente.
   - La victoria depende del formato: todos, el líder o la última oleada.
4. **Barra.** La del líder (escolta), la suma y la cuenta (manada) o dos barras (dúo).
5. **Motor de bendiciones.**
   - Hoy sigue a un solo rival (`Motor.ent`). Pasa a seguir al objetivo fijado, o al último que
     perdió vida.
   - Las marcas, los remates por debajo del 5 % y la barra de estados van con ese.
   - Lo que sale de golpear ya va a quien golpeas.
6. **Escalado.** El tramo del acto a todos. Factores de vida por formato y en cooperativo, en
   `data/escalado.json`.
7. **Validación.**
   - `tools/validar_msb.py` admitirá encuentros de varios enemigos.
   - Son 83 nuevos a unos 100 s cada uno: menos de 3 horas en segundo plano.

Cuánto trabajo, en sesiones:

| Paso | Qué | Sesiones |
|---|---|---|
| 1 | Datos y ModBuilder con varios huecos, evento por oleada, validador | 1 |
| 2 | Director: lista de rivales, formatos, barra y motor con objetivo | 2 |
| 3 | Pasar la propuesta a `data/acts` (actos 1, 2 y 3) y validar los nuevos | 1 (más la validación sola) |
| 4 | Campeón, racha, contrarreloj y aguante | 1 o 2 |
| 5 | Listas de cooperativo, escalado con Seamless y oleadas por jugador | 1 |
| 6 | Equilibrio del acto 3 con medidas, como se hizo con el acto 1 | 1 |

## Riesgos y abiertos

- **Jefes colocados ya validados (15-10, noche):** 131 de 132 van bien colocados en el MSB. La Virgen
  raptora con guadañas pendulares (16000860) se queda quieta: colocada no se ve, y con `spawn_chr`
  pega pero no persigue. Va sin variante, con `spawn_chr`. Propuesta: quitarla del acto 2.
- **Enemigos normales que no se mueven.** Algunos tienen IA de patrulla o de dormir y en el coliseo
  pueden quedarse quietos, como los 16 jefes. La validación los quita.
- **Las segundas fases de Malenia, Radahn consorte y Messmer** las ponen eventos del mapa original. En
  el coliseo puede que no salgan.
  - Así son señores más flojos de lo que parecen.
  - O se compensa con vida y mutadores, o se recrea la fase desde la DLL. **PROBAR** qué hace cada uno.
- **Bayle y Placidusax:** vuelan y se teletransportan; en el coliseo pequeño pueden quedar mal.
- **Los gigantes** (Radahn de Caelid, Gigante de fuego, Theodorix) piden otro coliseo o quedarse fuera.
- **Rendimiento:** probados 16 jefes a la vez en el coliseo sin problema. Con manadas de 5 sobra.
- **Arqueros y magos en manada** (academia, albináuricos) pueden ser pesados de pelear. Mirarlo al
  equilibrar.

## Ampliación y mezclas (24-10)

el jugador: «usando todo lo que esté fuera que puedas y dejando solo fuera lo justificado», y en cooperativo
«diseñando el algoritmo de mezclas». Editor de las tablas: página privada «Combates de RogueRing»
(tools/editor_combates.py; los cambios del jugador se guardan en su base de datos y Claude los aplica).

- **tools/ampliar_enemigos.py.** Los modelos de los alias de Smithbox que no salían en ningún encuentro:
  - Fuera, con motivo en `data/enemigos_fuera.json`: dragones voladores, gigantes, Rykard, Mímico,
    Tibia, el Carro de llamas, cadáveres y objetos, PNJ amistosos (equipo 26 del juego) y los que no
    tienen fila de NpcParam o de IA.
  - El resto, un `coliseo_<modelo>` con su fila base, en el acto y tipo cuya mediana de vida base se le
    parece más (los del DLC, en los actos 2 y 3; los jefes, élite o jefe). Los de menos de 400 de vida
    van en manada de tres («X ×3»). 59 nuevos, con la etiqueta `nuevo_24_10`.
- **Parejas de cooperativo, sorteadas en cada run.** el jugador: «un algoritmo vivo en el mod, que mezcle y
  genere parejas distintas cada vez que alguien juega una run nueva en coop» y «que funcione siempre».
  - tools/mezclas_coop.py escribe en data/coop/mezclas_<acto>.json todas las parejas posibles que
    cumplen las reglas (2092 el 24-10).
  - Reglas para que funcionen siempre:
    - dos combates de un enemigo del mismo acto y tipo, del coliseo, de modelo y nombre distintos;
    - vida base parecida (la mayor, como mucho el triple de la menor);
    - nunca dos grandes (radio de NpcParam de 3 m o más);
    - del mismo bando: ModBuilder pone a todos los enemigos de combate en el bando corriente
      (data/bandos.json), porque los del bando 7 y otros se atacaban a propósito entre ellos;
    - los golpes de rebote valen (el jugador: «daños accidentales no me importa»), como los hechizos de las
      reinas semihumanas;
    - fuera los marcados «No mezclar» en el editor y las parejas que fallan en el banco;
    - el Perro guardián entra: se queda de estatua hasta que te acercas, como en su mapa.
  - El mod (rogue_core::run::sortear_mezclas) saca al empezar cada acto de cooperativo unas cuantas por
    tipo:
    - tantas como enemigos distintos haya, hasta 30;
    - cada enemigo en dos como mucho;
    - con la semilla de la run, así que los dos PCs sacan las mismas y cada run son otras.

    Se juntan con las parejas hechas a mano (data/coop/<acto>.json) y los combates de solitario.
  - ModBuilder hace el coliseo de cada pareja posible: los enemigos colocados en el mapa son lo que
    sincroniza Seamless.
  - tools/probar_mezclas.py prueba en el banco una muestra con cada combinación de bandos y tamaños: que
    salgan, se muevan con el jugador al lado y se ganen; la vida que se quitan entre ellos sale como aviso.
    El 24-10 pasaron todas las de la muestra (132).
- **Vida:** tools/equilibrar_vida.py con la misma curva para cooperativo (COOP = 1.0): el +50 % por
  jugador extra lo pone el juego (`coop_extra`).
- **Validación:** tools/validar_msb.py con los ids nuevos y tools/aplicar_validacion.py quita los que
  fallan (y apunta el motivo).

## Jefes de dos fases con nuestro sistema (24-10)

el jugador: «comprobar todos los bosses con 2 fases si funcionan bien con nuestro sistema». Prueba en el banco:
`tools/probar_fases.py` (resultado en runtime/probar_fases.json), con la orden nueva del banco
`vida_chr <entidad> <vida>`.

- **Cómo se prueba cada sistema.**
  - `fase_en`: baja la vida bajo el umbral, mira el aviso, el cambio y que la segunda aparezca, se
    mueva y se gane.
  - `renacer`: mira que pase a su segunda fase y vuelva a vida llena.
  - `ultimo_en_pie`: mata al otro del dúo y mira la segunda fase del que queda.
  - Fase de su propia IA: le deja la vida al 45 % y apunta 30 s de animaciones y sus efectos y los
    del jugador.
- **Eventos de su mapa que faltaban.** Se buscaron en extract/event:
  - los eventos que miran la vida de un jefe (4[2]);
  - los que reaccionan a un efecto suyo (4[5]);
  - qué efectos de esos mira su IA (constantes de su `*_battle.lua`).

  Lo que su IA necesita y en el coliseo no le daba nadie:
  - **Messmer:** su segunda fase necesita el efecto 20010612 (su IA lo mira). Va en el campo nuevo
    `efecto` del enemigo de la segunda fase, que la DLL pone al aparecer.
  - **Godrick:** al ponerse él el 14785, su evento le pone el 14750 (casi toda su IA lo mira), cambia
    partes del modelo (la cabeza de dragón) y le pide la animación 20011.
  - **Malenia:** su segunda fase cambia partes del modelo y materiales.
  - **Mohg:** las fases del Nihil le ponen efectos al jugador (10650 y siguientes).
  - **Espíritu ancestral regio:** su curación termina en unos efectos suyos según los ciervos.
  - **Bestia de Elden:** su animación de entrada (20000).
- **data/reacciones.json.** Lo de Godrick, Malenia, Mohg y el Espíritu: ModBuilder copia las
  instrucciones del evento original a un evento por hueco del coliseo (y de las arenas grandes). La
  entidad del jefe pasa a ser la del hueco, con la misma condición «tiene el efecto». Al ser evento de
  mapa, en cooperativo corre en cada PC.
- **Lo que no se porta.** Mecánicas que dependen de otras piezas del mapa original:
  - los clones del Lamentador;
  - las posiciones de Maliketh en su arena;
  - los dedos de Metyr;
  - el árbol del Avatar de la Sombra;
  - los efectos del León danzante según el estado del jugador.

  Estos jefes pelean, pero sin esa parte.
