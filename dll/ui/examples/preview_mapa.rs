//! Vista previa del mapa sin abrir el juego: genera una run con los actos de data/acts y guarda PNG.
//! cargo run --release -p rogue-ui --example preview_mapa -- <semilla> <salida.png> [pasos] [acto]
//! COOP=1: un compañero eligiendo y otro con el destino votado.
#[path = "comun/mod.rs"]
mod comun;

use rogue_core::relics::RelicFile;
use rogue_core::run::{ActDef, RunState};
use rogue_ui::{bendiciones, mapa};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed = a.get(1).cloned().unwrap_or("ELDEN".into());
    let out = a.get(2).cloned().unwrap_or("mapa.png".into());
    let steps: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let acto: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut v = comun::Vista::new();
    let act: ActDef = serde_json::from_str(&std::fs::read_to_string(v.root.join(format!("data/acts/{acto}.json"))).unwrap()).unwrap();
    let acts = vec![act];
    let mut run = RunState::new(&seed, &acts);
    // Avanzar unos pasos por el primer camino disponible para ver el estado "recorrido".
    for _ in 0..steps {
        let next = run.available()[0];
        let _ = run.enter(next);
        let _ = run.complete(&acts);
    }
    let tex = v.mapa(&format!("extract/ui/fondos/acto{acto}.png"));
    let rel: RelicFile = serde_json::from_str(&std::fs::read_to_string(v.root.join("data/reliquias.json")).unwrap()).unwrap();
    let relics: Vec<bendiciones::BlessingView> = rel
        .reliquias
        .iter()
        .take(std::env::var("RELIQUIAS").ok().and_then(|n| n.parse().ok()).unwrap_or(7))
        .map(|r| bendiciones::BlessingView { icon: v.icono(r.icono), nombre: r.nombre.clone(), texto: r.niveles[0].texto.clone(), maldita: r.maldita })
        .collect();
    let mut view = mapa::MapView::from_run(&run, &acts[0].name, None);
    view.act_num = acto;
    view.viales = Some((2, 2));
    if let Ok(b) = std::env::var("JEFE") {
        view.boss_name = b;
    }
    view.arma = Some((12, 25));
    if std::env::var("COOP").is_ok() {
        let av = run.available();
        view.coop = true;
        view.cursores = vec![(av[av.len() - 1], "Mionel".into())];
        view.votos = vec![(av[0], "Ana".into())];
        view.espera = vec!["Mionel".into()];
        // MIVOTO=1: tú has votado el nodo del cursor y los demás aún no.
        if std::env::var("MIVOTO").is_ok() {
            view.mi_voto = view.selected;
            view.votos.clear();
            view.cursores.clear();
        }
    }
    // AVISOS=1: dos bendiciones recién ganadas, como en el juego.
    let avisos: Vec<(bendiciones::BlessingView, f32)> = if std::env::var("AVISOS").is_ok() {
        rel.reliquias.iter().skip(10).take(2).map(|r| (bendiciones::BlessingView { icon: None, nombre: r.nombre.clone(), texto: r.niveles[0].texto.clone(), maldita: false }, 1.0)).collect()
    } else {
        vec![]
    };
    v.render(&out, |ui, fonts| {
        mapa::draw(ui, &view, &tex, fonts, 0.3, &relics);
        bendiciones::draw_toasts(ui, &avisos, &tex.menu, fonts, true);
    });
    println!("{out}");
}
