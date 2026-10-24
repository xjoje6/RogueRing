//! Genera el mapa de un acto y lo imprime en JSON.
//! cargo run --example mapa -- <semilla> [sts2|rogue|data/acts/N.json] [acto]
use rogue_core::{map, rng};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed = a.get(1).cloned().unwrap_or_else(|| "ELDEN".into());
    let shape = match a.get(2).map(String::as_str) {
        Some("sts2") => map::ActShape::sts2_act1(),
        // La forma de un acto del mod (su «shape»).
        Some(p) if p.ends_with(".json") => {
            let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p).expect("no se lee el acto")).expect("acto mal formado");
            serde_json::from_value(v["shape"].clone()).expect("shape mal formado")
        }
        _ => map::ActShape::rogue_ring_default(),
    };
    let act = a.get(3).cloned().unwrap_or_else(|| "1".into());
    let mut r = rng::Rng::named(rng::hash_str(&seed), &format!("act_{act}_map"));
    let m = map::generate(&shape, &mut r);
    println!("{}", serde_json::to_string(&serde_json::json!({ "seed": seed, "act": act, "map": m })).unwrap());
}
