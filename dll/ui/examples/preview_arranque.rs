//! Vista previa de la carga del arranque (logo y gracia).
//! cargo run --release -p rogue-ui --example preview_arranque -- <salida.png> [segundos]
#[path = "comun/mod.rs"]
mod comun;

use rogue_ui::titulo;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t: f32 = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(3.0);
    let mut v = comun::Vista::new();
    let menu = v.menu();
    v.render(&a[1], |ui, _| titulo::carga(ui, &menu, t, 0.6));
}
