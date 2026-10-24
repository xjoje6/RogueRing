//! Vista previa de «Lo que llevas» con unas bendiciones de data/reliquias.json.
//! cargo run --release -p rogue-ui --example preview_llevas -- <salida.png> [cuántas] [elegida]
#[path = "comun/mod.rs"]
mod comun;

use rogue_core::relics::RelicFile;
use rogue_ui::bendiciones;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(7);
    let sel: usize = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(1);
    let mut v = comun::Vista::new();
    let tex = v.mapa("extract/ui/fondos/acto1.png");
    let rel: RelicFile = serde_json::from_str(&std::fs::read_to_string(v.root.join("data/reliquias.json")).unwrap()).unwrap();
    let items: Vec<bendiciones::BlessingView> = rel
        .reliquias
        .iter()
        .take(n)
        .map(|r| bendiciones::BlessingView { icon: v.icono(r.icono), nombre: r.nombre.clone(), texto: r.niveles[0].texto.clone(), maldita: r.maldita })
        .collect();
    v.render(&a[1], |ui, fonts| bendiciones::draw_list(ui, &items, sel, 5.0, &tex, fonts));
}
