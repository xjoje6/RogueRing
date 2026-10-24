"""Página del editor de combates (revision_enemigos/editor.html) con los datos y las imágenes dentro.

Uso: python tools/enemigos.py && python tools/editor_combates.py

Se publica como página privada de claude.ai con su base de datos (capacidad `db`): ahí se guardan los
cambios del jugador (colección `cambios`, uno por encuentro: lista, acto, tipo, quitar, nota) y sus propuestas
de enemigos sin usar (colección `propuestas`, una por modelo: lista, acto, tipo, nota). Claude los lee y
los aplica a data/acts y data/coop (tools/aplicar_editor.py).

También escribe revision_enemigos/editor_para_compartir.html: el mismo editor como página suelta, para
abrirla con doble clic sin claude.ai (24-10: pasársela a un amigo). Ahí los cambios se guardan en
el navegador y «Descargar mis cambios» da un .json con autor y fecha, que se aplica igual con
tools/aplicar_editor.py.
"""
import base64
import json
import os

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
DIR = os.path.join(ROOT, "revision_enemigos")


def main():
    d = json.load(open(os.path.join(DIR, "enemigos.json"), encoding="utf8"))
    imgs = {}
    for nombre, archivo in d["imagenes"].items():
        if archivo and os.path.exists(os.path.join(DIR, "img", archivo)):
            imgs[nombre] = "data:image/jpeg;base64," + base64.b64encode(open(os.path.join(DIR, "img", archivo), "rb").read()).decode()
    datos = {"usados": d["usados"], "sin_usar": d["sin_usar"], "mezclas": d.get("mezclas", {})}
    html = PLANTILLA.replace("__DATOS__", json.dumps(datos, ensure_ascii=False)).replace("__IMAGENES__", json.dumps(imgs))
    open(os.path.join(DIR, "editor.html"), "w", encoding="utf8").write(html)
    # Suelta: con su esqueleto (sin él, el navegador no sabe que es UTF-8 y la abre en modo antiguo).
    suelta = ('<!doctype html><html lang="es"><head><meta charset="utf-8">'
              '<meta name="viewport" content="width=device-width,initial-scale=1"></head><body>' + html + "</body></html>")
    open(os.path.join(DIR, "editor_para_compartir.html"), "w", encoding="utf8").write(suelta)
    print(f"editor.html: {len(html) / 1e6:.1f} MB, {len(d['usados'])} encuentros, {len(d['sin_usar'])} sin usar, {len(imgs)} imágenes")


PLANTILLA = r"""<title>Combates de RogueRing</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Marcellus&family=Source+Sans+3:wght@400;600&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
/* Mesa de trabajo: cabecera fija con los filtros, tablero de columnas por tipo de combate y, a la derecha,
   la lista de cambios. Piedra fría y verdete, como el bronce viejo de las Tierras Intermedias. */
:root {
  --bg: #e6e8e3; --panel: #f2f3ef; --ink: #1c201e; --muted: #5d645f; --line: #c6cbc3;
  --accent: #2c6a5d; --accent-ink: #f2f3ef; --moved: #8d5711; --removed: #9a2d2d; --new: #2c5a8a;
  --display: "Marcellus", "Trajan Pro", Georgia, serif;
  --body: "Source Sans 3", "Segoe UI", system-ui, sans-serif;
  --mono: "IBM Plex Mono", ui-monospace, Consolas, monospace;
}
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) {
  --bg: #121513; --panel: #1a1e1c; --ink: #d9ddd6; --muted: #8d948e; --line: #2d332f;
  --accent: #79b7a6; --accent-ink: #121513; --moved: #d9a04e; --removed: #e27b6f; --new: #7fb0e0; color-scheme: dark } }
:root[data-theme="dark"] {
  --bg: #121513; --panel: #1a1e1c; --ink: #d9ddd6; --muted: #8d948e; --line: #2d332f;
  --accent: #79b7a6; --accent-ink: #121513; --moved: #d9a04e; --removed: #e27b6f; --new: #7fb0e0; color-scheme: dark }
body { background: var(--bg); color: var(--ink); font: 15px/1.45 var(--body); }
body::before { content: ""; position: fixed; inset: 0 0 auto; height: env(safe-area-inset-top, 0px); z-index: 10; background: var(--bg); }
.wrap { padding-left: max(16px, env(safe-area-inset-left, 0px)); padding-right: max(16px, env(safe-area-inset-right, 0px)); padding-block: 0 48px; }
header { position: sticky; top: env(safe-area-inset-top, 0px); z-index: 5; background: var(--bg); padding-block: 18px 12px; border-bottom: 1px solid var(--line); display: grid; gap: 12px; }
h1 { font: 400 clamp(26px, 4vw, 38px)/1.05 var(--display); margin: 0; letter-spacing: .01em; text-wrap: balance; }
.top { display: flex; flex-wrap: wrap; gap: 8px 20px; align-items: baseline; justify-content: space-between; }
.lead { color: var(--muted); margin: 0; max-width: 62ch; }
.estado { font: 500 12px var(--mono); color: var(--muted); }
.estado.ok { color: var(--accent); } .estado.mal { color: var(--removed); }
.filtros { display: flex; flex-wrap: wrap; gap: 10px 18px; align-items: center; }
.seg { display: inline-flex; border: 1px solid var(--line); }
.seg button { font: 600 13px var(--body); padding: 6px 12px; background: transparent; color: var(--ink); border: 0; border-right: 1px solid var(--line); cursor: pointer; }
.seg button:last-child { border-right: 0; }
.seg button[aria-pressed="true"] { background: var(--accent); color: var(--accent-ink); }
.seg button.drop { outline: 2px dashed var(--moved); outline-offset: -3px; }
input[type=search], input[type=text], select, textarea { font: 14px var(--body); color: var(--ink); background: var(--panel); border: 1px solid var(--line); padding: 5px 8px; }
input[type=search] { min-width: 0; width: 220px; max-width: 100%; }
button:focus-visible, select:focus-visible, input:focus-visible, textarea:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.cuerpo { display: grid; grid-template-columns: minmax(0, 1fr) 290px; gap: 24px; margin-top: 18px; }
@media (max-width: 900px) { .cuerpo { grid-template-columns: minmax(0, 1fr); } }
.tablero { display: grid; grid-template-columns: repeat(auto-fit, minmax(230px, 1fr)); gap: 16px; align-items: start; }
.col { min-width: 0; display: grid; gap: 8px; padding: 10px; border: 1px solid var(--line); min-height: 120px; }
.col.drop { border-color: var(--moved); background: color-mix(in srgb, var(--moved) 7%, transparent); }
.col h2 { font: 400 19px var(--display); margin: 0 0 2px; display: flex; justify-content: space-between; align-items: baseline; }
.col h2 small { font: 500 12px var(--mono); color: var(--muted); }
.carta { background: var(--panel); border: 1px solid var(--line); padding: 8px; display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 4px 10px; cursor: grab; }
.carta[data-marca="movido"] { border-left: 4px solid var(--moved); }
.carta[data-marca="quitado"] { border-left: 4px solid var(--removed); opacity: .62; }
.carta[data-marca="nuevo"] { border-left: 4px solid var(--new); }
.carta[data-marca="nomezcla"] { border-left: 4px solid var(--muted); }
.sub { display: grid; gap: 4px; margin-top: 6px; padding-top: 8px; border-top: 1px solid var(--line); }
.sub h3 { font: 600 12px var(--body); color: var(--muted); margin: 0 0 2px; letter-spacing: .02em; }
.fila-m { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 6px; align-items: center; font-size: 13px; }
.fila-m[data-fuera] .n { text-decoration: line-through; color: var(--muted); }
.fila-m .n { overflow-wrap: anywhere; }
.fila-m .boton { font-size: 11px; padding: 1px 7px; }
.lead + .tablero { margin-top: 12px; }
.caras { display: flex; grid-row: span 2; }
.caras img, .caras span { width: 52px; height: 52px; object-fit: cover; display: block; background: var(--line); }
.caras img + img { margin-left: -18px; outline: 2px solid var(--panel); }
.caras span { display: grid; place-items: center; font: 500 11px var(--mono); color: var(--muted); }
.nombre { font-weight: 600; line-height: 1.2; overflow-wrap: anywhere; }
.meta { font: 12px var(--mono); color: var(--muted); }
.acciones { grid-column: 1 / -1; display: flex; flex-wrap: wrap; gap: 6px; align-items: center; margin-top: 4px; }
.acciones select { font-size: 12px; padding: 2px 4px; }
.acciones button, .boton { font: 600 12px var(--body); padding: 3px 9px; border: 1px solid var(--line); background: transparent; color: var(--ink); cursor: pointer; }
.boton.prim { background: var(--accent); color: var(--accent-ink); border-color: var(--accent); }
.lado { display: grid; gap: 10px; align-content: start; min-width: 0; }
.lado h2 { font: 400 20px var(--display); margin: 0; }
.cambio { font-size: 13px; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 6px; border-bottom: 1px solid var(--line); padding-bottom: 6px; }
.cambio b { font-weight: 600; } .cambio .q { color: var(--muted); }
.vacio { color: var(--muted); font-size: 13px; }
.rejilla { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
.ficha { background: var(--panel); border: 1px solid var(--line); padding: 10px; display: grid; grid-template-columns: 72px minmax(0, 1fr); gap: 6px 12px; }
.ficha img, .ficha .sin { width: 72px; height: 72px; object-fit: cover; background: var(--line); grid-row: span 3; }
.ficha .motivo { font-size: 13px; color: var(--muted); }
.ficha form { grid-column: 1 / -1; display: grid; gap: 6px; border-top: 1px solid var(--line); padding-top: 8px; }
.ficha form .fila { display: flex; flex-wrap: wrap; gap: 6px; }
.ficha textarea { min-height: 70px; resize: vertical; }
.ficha[data-propuesta] { border-left: 4px solid var(--new); }
.etq { font: 500 11px var(--mono); color: var(--muted); letter-spacing: .04em; text-transform: uppercase; }
@media (prefers-reduced-motion: no-preference) { .carta, .col { transition: border-color .15s, background .15s; } }
</style>

<div class="wrap">
<header>
  <div class="top">
    <h1>Combates de RogueRing</h1>
    <span class="estado" id="estado">Cargando tus cambios…</span>
  </div>
  <p class="lead">Arrastra un combate a otra columna para cambiarle el tipo, o sobre un acto para llevarlo allí. Cada carta tiene también sus selectores. Los cambios se guardan solos y Claude los lee para pasarlos al juego.</p>
  <div class="filtros">
    <div class="seg" role="group" aria-label="Lista">
      <button id="l-acts" data-lista="acts">Solitario</button><button id="l-coop" data-lista="coop">Cooperativo</button><button id="l-sin" data-lista="sin">Sin usar</button>
    </div>
    <div class="seg" role="group" aria-label="Acto" id="actos">
      <button data-acto="1" id="a1">Acto I</button><button data-acto="2" id="a2">Acto II</button><button data-acto="3" id="a3">Acto III</button>
    </div>
    <input type="search" id="buscar" placeholder="Buscar enemigo" aria-label="Buscar enemigo">
  </div>
  <div class="filtros" id="suelta" hidden>
    <label>Tu nombre <input type="text" id="autor" autocomplete="name" placeholder="Para saber de quién son"></label>
    <button type="button" class="boton prim" id="descargar">Descargar mis cambios</button>
    <span class="meta">Se guardan en este navegador. Al acabar, descárgalos y pásale el archivo al jugador.</span>
  </div>
</header>
<div class="cuerpo">
  <main id="vista" aria-live="polite"></main>
  <aside class="lado">
    <h2>Cambios</h2>
    <div id="cambios"></div>
  </aside>
</div>
</div>

<script>
const DATOS = __DATOS__;
const IMG = __IMAGENES__;
const TIPOS = { Weak: "Débil", Normal: "Normal", Elite: "Élite", Boss: "Jefe" };
const ROMANO = { 1: "I", 2: "II", 3: "III" };
const LISTAS = { acts: "Solitario", coop: "Cooperativo" };
const st = { lista: "acts", acto: "1", buscar: "", cambios: {}, propuestas: {}, abierto: null };
let db = null, suelta = false;
const LOCAL = "combates-cambios-local";
function guardarLocal() {
  try { localStorage.setItem(LOCAL, JSON.stringify({ cambios: st.cambios, propuestas: st.propuestas })); estado("Guardado en este navegador", "ok"); }
  catch (e) { estado("Este navegador no deja guardar: descarga tus cambios antes de cerrar", "mal"); }
}
try { const g = JSON.parse(localStorage.getItem("combates-vista") || "{}"); Object.assign(st, { lista: g.lista || st.lista, acto: g.acto || st.acto }); } catch (e) {}

const porId = new Map(DATOS.usados.map(e => [e.lista + ":" + e.id, e]));
const clave = e => e.lista + ":" + e.id;
const docId = k => k.replace(/[^A-Za-z0-9_\-.~:@+]/g, "_");
const nom = x => x.nombre_es || x.nombre;
const el = (t, a = {}, ...h) => { const n = document.createElement(t); for (const [k, v] of Object.entries(a)) { if (k === "class") n.className = v; else if (k.startsWith("on")) n.addEventListener(k.slice(2), v); else if (v !== undefined && v !== null && v !== false) n.setAttribute(k, v === true ? "" : v); } for (const c of h.flat()) if (c != null) n.append(c.nodeType ? c : document.createTextNode(c)); return n; };

function efectivo(e) {
  const c = st.cambios[docId(clave(e))];
  return { acto: c?.acto ?? e.acto, tipo: c?.tipo ?? e.tipo, quitar: !!c?.quitar, no_mezclar: !!c?.no_mezclar, cambio: c };
}
function marca(e, ef) {
  if (ef.quitar) return "quitado";
  if (ef.acto !== e.acto || ef.tipo !== e.tipo) return "movido";
  if (ef.no_mezclar) return "nomezcla";
  return "";
}
function estado(t, cl = "") { const s = document.getElementById("estado"); s.textContent = t; s.className = "estado " + cl; }

async function guardarCambio(e, cambio) {
  const id = docId(clave(e));
  const base = { acto: e.acto, tipo: e.tipo, quitar: false };
  const limpio = { acto: cambio.acto, tipo: cambio.tipo, quitar: !!cambio.quitar, no_mezclar: !!cambio.no_mezclar };
  const igual = limpio.acto === base.acto && limpio.tipo === base.tipo && !limpio.quitar && !limpio.no_mezclar;
  if (igual) delete st.cambios[id]; else st.cambios[id] = { ...limpio, lista: e.lista, id: e.id, nombre: e.nombre, antes: base };
  pintar();
  if (suelta) return guardarLocal();
  if (!db) { estado("Sin guardado: ábrela en claude.ai para que se guarde", "mal"); return; }
  try {
    estado("Guardando…");
    if (igual) await db.collection("cambios").doc(id).delete();
    else await db.collection("cambios").doc(id).set(st.cambios[id]);
    estado("Guardado", "ok");
  } catch (err) { estado("No se pudo guardar: " + (err.message || err.code), "mal"); }
}
async function guardarPropuesta(s, p) {
  if (p) st.propuestas[s.chr] = { ...p, chr: s.chr, nombre: s.nombre }; else delete st.propuestas[s.chr];
  st.abierto = null; pintar();
  if (suelta) return guardarLocal();
  if (!db) { estado("Sin guardado: ábrela en claude.ai para que se guarde", "mal"); return; }
  try {
    estado("Guardando…");
    if (p) await db.collection("propuestas").doc(s.chr).set(st.propuestas[s.chr]);
    else await db.collection("propuestas").doc(s.chr).delete();
    estado("Guardado", "ok");
  } catch (err) { estado("No se pudo guardar: " + (err.message || err.code), "mal"); }
}

function caras(enemigos) {
  const vistos = [], out = el("div", { class: "caras" });
  for (const x of enemigos) { if (vistos.includes(x.nombre)) continue; vistos.push(x.nombre); }
  for (const n of vistos.slice(0, 3)) out.append(IMG[n] ? el("img", { src: IMG[n], alt: "" }) : el("span", {}, "?"));
  return out;
}

function carta(e) {
  const ef = efectivo(e), m = marca(e, ef);
  const n = {}; for (const x of e.enemigos) n[nom(x)] = (n[nom(x)] || 0) + 1;
  const quienes = Object.entries(n).map(([k, v]) => v > 1 ? v + " × " + k : k).join(", ");
  const selActo = el("select", { "aria-label": "Acto", id: "sa-" + docId(clave(e)), onchange: ev => guardarCambio(e, { ...ef, acto: ev.target.value }) },
    ["1", "2", "3"].map(a => el("option", { value: a, selected: a === ef.acto }, "Acto " + ROMANO[a])));
  const tipos = e.lista === "coop" ? ["Normal", "Elite", "Boss"] : ["Weak", "Normal", "Elite", "Boss"];
  const selTipo = el("select", { "aria-label": "Tipo", id: "st-" + docId(clave(e)), onchange: ev => guardarCambio(e, { ...ef, tipo: ev.target.value }) },
    tipos.map(t => el("option", { value: t, selected: t === ef.tipo }, TIPOS[t])));
  const antes = m === "movido" ? "antes: acto " + ROMANO[e.acto] + ", " + TIPOS[e.tipo] : "";
  const c = el("article", { class: "carta", draggable: "true", "data-marca": m, "data-k": clave(e),
      ondragstart: ev => { ev.dataTransfer.setData("text/plain", clave(e)); ev.dataTransfer.effectAllowed = "move"; } },
    caras(e.enemigos),
    el("div", { class: "nombre" }, e.nombre_es || e.nombre),
    el("div", { class: "meta" }, [e.arena ? "arena grande" : "coliseo", ef.no_mezclar ? "no se mezcla en cooperativo" : null, e.nuevo ? "nuevo" : null, e.formato !== "solo" && !e.mezcla ? e.formato : null, e.vida ? "vida ×" + e.vida : null, e.enemigos.length > 1 ? quienes : null, antes || null].filter(Boolean).join(" · ") || " "),
    el("div", { class: "acciones" }, selActo, selTipo,
      el("button", { type: "button", onclick: () => guardarCambio(e, { ...ef, quitar: !ef.quitar }) }, ef.quitar ? "Volver a meter" : "Quitar"),
      e.mezclable || ef.no_mezclar ? el("button", { type: "button", title: "Si entra o no en las parejas que sortea el cooperativo", onclick: () => guardarCambio(e, { ...ef, no_mezclar: !ef.no_mezclar }) }, ef.no_mezclar ? "Volver a mezclar" : "No mezclar") : null,
      m ? el("button", { type: "button", onclick: () => guardarCambio(e, { acto: e.acto, tipo: e.tipo, quitar: false }) }, "Deshacer") : null));
  return c;
}

function zona(nodo, alSoltar) {
  nodo.addEventListener("dragover", ev => { ev.preventDefault(); nodo.classList.add("drop"); });
  nodo.addEventListener("dragleave", () => nodo.classList.remove("drop"));
  nodo.addEventListener("drop", ev => { ev.preventDefault(); nodo.classList.remove("drop"); const e = porId.get(ev.dataTransfer.getData("text/plain")); if (e) alSoltar(e); });
}

function pintarTablero(v) {
  // En cooperativo, la columna de débiles solo si en ese acto hay algo (grupos o parejas sorteables).
  const debiles = st.lista !== "coop" || (DATOS.mezclas[st.acto] || {}).Weak || DATOS.usados.some(e => e.lista === "acts" && e.acto === st.acto && e.tipo === "Weak" && e.enemigos.length > 1);
  const tipos = debiles ? ["Weak", "Normal", "Elite", "Boss"] : ["Normal", "Elite", "Boss"];
  const q = st.buscar.toLowerCase();
  const lista = DATOS.usados.filter(e => e.lista === st.lista).filter(e => efectivo(e).acto === st.acto)
    .filter(e => !q || [e.nombre, e.nombre_es, ...e.enemigos.flatMap(x => [x.nombre, x.nombre_es])].some(t => (t || "").toLowerCase().includes(q)));
  if (st.lista === "coop") {
    const m = DATOS.mezclas[st.acto] || {}, total = Object.values(m).reduce((a, b) => a + b, 0);
    v.append(el("p", { class: "lead" }, "En cooperativo nunca sale un enemigo solo. Cada run, al empezar el acto, el juego sortea parejas entre los enemigos de «Se sortean en parejas» de cada columna: dos distintos del mismo tipo y con vida parecida. En este acto hay " + total + " combinaciones posibles (" + Object.entries(m).map(([k, n]) => TIPOS[k] + " " + n).join(", ") + "). Con ellas salen las parejas hechas a mano (las cartas) y los grupos de solitario, con más vida por cada jugador. Cada combate va al coliseo o a la arena grande según el tamaño de lo que sale a la vez. «No mezclar» saca a un enemigo del sorteo."));
  }
  const t = el("div", { class: "tablero" });
  for (const tipo of tipos) {
    const de = lista.filter(e => efectivo(e).tipo === tipo);
    const col = el("section", { class: "col", "aria-label": TIPOS[tipo] }, el("h2", {}, TIPOS[tipo], el("small", {}, de.length + " combates")));
    if (!de.length) col.append(el("p", { class: "vacio" }, q ? "Nada con ese nombre." : "Vacía. Arrastra aquí un combate."));
    for (const e of de) col.append(carta(e));
    if (st.lista === "coop") col.append(...apartadosCoop(tipo, q));
    zona(col, e => guardarCambio(e, { ...efectivo(e), tipo }));
    t.append(col);
  }
  v.append(t);
}

// Cooperativo: además de las parejas hechas a mano, los grupos de solitario de ese acto y tipo, y los
// enemigos de uno que el juego sortea en parejas en cada run (con «No mezclar»).
function apartadosCoop(tipo, q) {
  const del = e => e.lista === "acts" && efectivo(e).acto === st.acto && efectivo(e).tipo === tipo && !efectivo(e).quitar
    && (!q || [e.nombre, e.nombre_es, ...e.enemigos.flatMap(x => [x.nombre, x.nombre_es])].some(t => (t || "").toLowerCase().includes(q)));
  const grupos = DATOS.usados.filter(e => del(e) && e.enemigos.length > 1);
  const mezcla = DATOS.usados.filter(e => del(e) && e.enemigos.length === 1 && e.mezclable);
  const out = [];
  if (grupos.length) {
    out.push(el("div", { class: "sub" }, el("h3", {}, "Grupos de solitario (" + grupos.length + ")"),
      grupos.map(e => el("div", { class: "fila-m" }, el("span", { class: "n" }, nom(e) + (e.arena ? " · arena grande" : "")), el("span"))))); 
  }
  if (mezcla.length) {
    const dentro = mezcla.filter(e => !efectivo(e).no_mezclar).length;
    out.push(el("div", { class: "sub" }, el("h3", {}, "Se sortean en parejas (" + dentro + " de " + mezcla.length + ")"),
      mezcla.map(e => {
        const ef = efectivo(e);
        return el("div", { class: "fila-m", "data-fuera": ef.no_mezclar ? "" : null }, el("span", { class: "n" }, nom(e)),
          el("button", { class: "boton", type: "button", onclick: () => guardarCambio(e, { ...ef, no_mezclar: !ef.no_mezclar }) }, ef.no_mezclar ? "Volver a mezclar" : "No mezclar"));
      })));
  }
  return out;
}

function pintarSinUsar(v) {
  const q = st.buscar.toLowerCase();
  const lista = DATOS.sin_usar.filter(s => !q || [s.nombre, s.nombre_es].some(t => (t || "").toLowerCase().includes(q)));
  v.append(el("p", { class: "lead" }, lista.length + " modelos del juego que no salen en ninguna tabla. Si quieres meter uno, dale a «Proponer», di dónde y por qué, y cómo evitar el problema que lo dejó fuera. Lo evalúo antes de meterlo."));
  const g = el("div", { class: "rejilla" });
  for (const s of lista) {
    const p = st.propuestas[s.chr];
    const f = el("article", { class: "ficha", "data-propuesta": p ? "" : null },
      IMG[s.nombre] ? el("img", { src: IMG[s.nombre], alt: "" }) : el("div", { class: "sin" }),
      el("div", { class: "nombre" }, nom(s)),
      el("div", { class: "etq" }, s.chr + (s.jefe ? " · jefe" : "") + (s.dlc ? " · DLC" : "")),
      el("div", { class: "motivo" }, s.motivo));
    if (st.abierto === s.chr) {
      const idb = "f-" + s.chr, v0 = p || { lista: "acts", acto: "1", tipo: "Normal", nota: "" };
      const fl = el("select", { id: idb + "-l" }, Object.entries(LISTAS).map(([k, n]) => el("option", { value: k, selected: k === v0.lista }, n)));
      const fa = el("select", { id: idb + "-a" }, ["1", "2", "3"].map(a => el("option", { value: a, selected: a === v0.acto }, "Acto " + ROMANO[a])));
      const ft = el("select", { id: idb + "-t" }, Object.entries(TIPOS).map(([k, n]) => el("option", { value: k, selected: k === v0.tipo }, n)));
      const fn = el("textarea", { id: idb + "-n", placeholder: "Por qué lo meterías y cómo evitar el problema que lo dejó fuera" }, v0.nota || "");
      f.append(el("form", { onsubmit: ev => { ev.preventDefault(); guardarPropuesta(s, { lista: fl.value, acto: fa.value, tipo: ft.value, nota: fn.value.trim() }); } },
        el("div", { class: "fila" }, fl, fa, ft), fn,
        el("div", { class: "fila" }, el("button", { class: "boton prim", type: "submit" }, "Guardar propuesta"),
          el("button", { class: "boton", type: "button", onclick: () => { st.abierto = null; pintar(); } }, "Cancelar"),
          p ? el("button", { class: "boton", type: "button", onclick: () => guardarPropuesta(s, null) }, "Retirar propuesta") : null)));
    } else {
      f.append(el("div", { class: "acciones" },
        p ? el("span", { class: "meta" }, "Propuesto: " + LISTAS[p.lista] + ", acto " + ROMANO[p.acto] + ", " + TIPOS[p.tipo]) : null,
        el("button", { type: "button", onclick: () => { st.abierto = s.chr; pintar(); } }, p ? "Editar propuesta" : "Proponer")));
    }
    g.append(f);
  }
  v.append(g);
}

function pintarCambios() {
  const c = document.getElementById("cambios"); c.replaceChildren();
  const filas = Object.entries(st.cambios);
  const props = Object.values(st.propuestas);
  if (!filas.length && !props.length) { c.append(el("p", { class: "vacio" }, "Aún no has cambiado nada. Lo que muevas, quites o propongas aparece aquí.")); return; }
  for (const [id, x] of filas) {
    const e = porId.get(x.lista + ":" + x.id); if (!e) continue;
    const movido = x.acto !== x.antes.acto || x.tipo !== x.antes.tipo;
    const que = x.quitar ? "quitado" : [movido ? "acto " + ROMANO[x.antes.acto] + " " + TIPOS[x.antes.tipo] + " → acto " + ROMANO[x.acto] + " " + TIPOS[x.tipo] : null, x.no_mezclar ? "no se mezcla" : null].filter(Boolean).join(" · ");
    c.append(el("div", { class: "cambio" }, el("div", {}, el("b", {}, e.nombre_es || e.nombre), el("div", { class: "q" }, LISTAS[e.lista] + ": " + que)),
      el("button", { class: "boton", type: "button", onclick: () => guardarCambio(e, { acto: e.acto, tipo: e.tipo, quitar: false }) }, "Deshacer")));
  }
  for (const p of props) c.append(el("div", { class: "cambio" }, el("div", {}, el("b", {}, p.nombre), el("div", { class: "q" }, "propuesto para " + LISTAS[p.lista] + ", acto " + ROMANO[p.acto] + ", " + TIPOS[p.tipo]))));
}

function pintar() {
  for (const b of document.querySelectorAll("[data-lista]")) b.setAttribute("aria-pressed", String(b.dataset.lista === st.lista));
  for (const b of document.querySelectorAll("[data-acto]")) b.setAttribute("aria-pressed", String(b.dataset.acto === st.acto));
  document.getElementById("actos").hidden = st.lista === "sin";
  const v = document.getElementById("vista"); v.replaceChildren();
  if (st.lista === "sin") pintarSinUsar(v); else pintarTablero(v);
  pintarCambios();
  try { localStorage.setItem("combates-vista", JSON.stringify({ lista: st.lista, acto: st.acto })); } catch (e) {}
}

for (const b of document.querySelectorAll("[data-lista]")) b.addEventListener("click", () => { st.lista = b.dataset.lista; pintar(); });
for (const b of document.querySelectorAll("[data-acto]")) {
  b.addEventListener("click", () => { st.acto = b.dataset.acto; pintar(); });
  zona(b, e => guardarCambio(e, { ...efectivo(e), acto: b.dataset.acto }));
}
document.getElementById("buscar").addEventListener("input", ev => { st.buscar = ev.target.value.trim(); pintar(); });
pintar();

function modoSuelto() {
  suelta = true;
  document.getElementById("suelta").hidden = false;
  try {
    const g = JSON.parse(localStorage.getItem(LOCAL) || "{}");
    st.cambios = g.cambios || {}; st.propuestas = g.propuestas || {};
    document.getElementById("autor").value = localStorage.getItem(LOCAL + "-autor") || "";
  } catch (e) {}
  pintar();
  estado(Object.keys(st.cambios).length + Object.keys(st.propuestas).length ? "Tus cambios de antes, cargados" : "Lo que cambies se guarda en este navegador", "ok");
}
document.getElementById("autor").addEventListener("input", ev => { try { localStorage.setItem(LOCAL + "-autor", ev.target.value.trim()); } catch (e) {} });
document.getElementById("descargar").addEventListener("click", () => {
  const autor = document.getElementById("autor").value.trim();
  if (!autor) { estado("Pon tu nombre antes de descargar", "mal"); document.getElementById("autor").focus(); return; }
  const h = new Date(), dos = n => String(n).padStart(2, "0");
  const fecha = h.getFullYear() + "-" + dos(h.getMonth() + 1) + "-" + dos(h.getDate()) + " " + dos(h.getHours()) + ":" + dos(h.getMinutes());
  const datos = { autor, fecha, cambios: st.cambios, propuestas: st.propuestas };
  const a = el("a", { href: URL.createObjectURL(new Blob([JSON.stringify(datos, null, 1)], { type: "application/json" })),
    download: "cambios_combates_" + autor.replace(/[^A-Za-z0-9_-]+/g, "_") + "_" + fecha.slice(0, 10) + ".json" });
  document.body.append(a); a.click(); a.remove();
  estado("Descargado: pásale el archivo al jugador", "ok");
});

(async () => {
  // Abierta con doble clic, sin claude.ai: modo suelto (guardado en el navegador y descarga).
  if (!window.claude) return modoSuelto();
  db = await window.claude.use("db");
  if (!db) { estado("Sin guardado: ábrela en claude.ai para que se guarde", "mal"); return; }
  const leer = (col, alLlegar) => db.collection(col).onSnapshot(s => { alLlegar(s); pintar(); estado("Al día", "ok"); }, e => estado("Sin conexión con tus cambios (" + e.code + ")", "mal"));
  leer("cambios", s => { st.cambios = {}; for (const d of s.docs) st.cambios[d.id] = { ...d.data() }; });
  leer("propuestas", s => { st.propuestas = {}; for (const d of s.docs) st.propuestas[d.id] = { ...d.data() }; });
})();
</script>
"""

if __name__ == "__main__":
    main()
