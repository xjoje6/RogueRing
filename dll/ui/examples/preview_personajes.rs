//! Vista previa de la pantalla de Personajes (el Invocador, con candados), sobre una pantalla de carga
//! que hace de escena.
//! cargo run --release -p rogue-ui --example preview_personajes -- <salida.png>
#[path = "comun/mod.rs"]
mod comun;

use rogue_ui::personajes;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut v = comun::Vista::new();
    let tex = v.mapa("extract/ui/fondos/acto1.png");
    let escena = v.load("extract/ui/carga/MENU_Load_00010_PS5.png").unwrap();
    let pool: Vec<rogue_core::rewards::Item> = serde_json::from_str(&std::fs::read_to_string(v.root.join("data/objetos/pool.json")).unwrap()).unwrap();
    let mut arma = |id: i32, candado: Option<&str>| {
        let it = pool.iter().find(|i| i.id == id);
        let icono = it.map(|i| i.icono).unwrap_or(0);
        (it.map(|i| i.nombre.clone()).unwrap_or_default(), it.map(|i| i.tipo.clone()).unwrap_or_default(), candado.map(String::from), v.icono(icono))
    };
    let armas = vec![arma(22000000, None), arma(22020000, Some("Win 3 runs with the Summoner (0/3)")), arma(22030000, Some("Win 10 runs with the Summoner (0/10)"))];
    let view = personajes::PersonajesView {
        nombre: "Summoner".into(),
        lema: "Never fights alone.".into(),
        pasiva: "Starts every fight with their summon.".into(),
        armas,
        arma_sel: 0,
        apariencias: vec![("Default".into(), None), ("Reborn".into(), Some("Win 1 runs with the Summoner (0/1)".into()))],
        apariencia_sel: 0,
        invocaciones: vec![
            ("Lone Wolf Ashes".into(), None),
            ("Banished Knight Oleg".into(), Some("Win 3 runs with the Summoner (0/3)".into())),
            ("Black Knife Tiche".into(), Some("Win 10 runs with the Summoner (0/10)".into())),
        ],
        invocacion_sel: 0,
        idx: 3,
        total: 4,
        t: 5.0,
        t_sel: 5.0,
    };
    v.render(&a[1], |ui, fonts| {
        ui.get_background_draw_list().add_image(escena.0, [0.0, 0.0], [1920.0, 1080.0]).build();
        personajes::draw(ui, &view, &tex, fonts)
    });
}
