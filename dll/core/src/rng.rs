//! RNG determinista con semilla de texto y un flujo por propósito, como `RunRngSet` de StS2:
//! elegir otra recompensa no debe cambiar el mapa ni los bosses.

/// Hash FNV-1a de 64 bits: estable entre versiones y plataformas (no usar el Hash de std).
pub fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// xoshiro256**. Serializable (4 palabras) para guardar la run a mitad.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    /// Flujo con nombre derivado de la semilla de la run, p. ej. `Rng::named(seed, "act_1_map")`.
    pub fn named(run_seed: u64, name: &str) -> Self {
        let mut st = run_seed ^ hash_str(name);
        Rng { s: [splitmix64(&mut st), splitmix64(&mut st), splitmix64(&mut st), splitmix64(&mut st)] }
    }

    pub fn next_u64(&mut self) -> u64 {
        let r = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        r
    }

    /// Entero en [lo, hi) sin sesgo apreciable.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        assert!(hi > lo);
        let span = (hi - lo) as u64;
        lo + ((self.next_u64() >> 11) % span) as i32
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.range(0, i as i32 + 1) as usize;
            v.swap(i, j);
        }
    }

    /// Gaussiana entera acotada, como `NextGaussianInt(media, desviación, mín, máx)` de StS2.
    pub fn gaussian_int(&mut self, mean: f64, sd: f64, min: i32, max: i32) -> i32 {
        let (u1, u2) = (self.next_f64().max(1e-12), self.next_f64());
        let z = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
        ((mean + z * sd).round() as i32).clamp(min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn misma_semilla_mismo_resultado() {
        let s = hash_str("ELDEN");
        let a: Vec<u64> = {
            let mut r = Rng::named(s, "map");
            (0..5).map(|_| r.next_u64()).collect()
        };
        let b: Vec<u64> = {
            let mut r = Rng::named(s, "map");
            (0..5).map(|_| r.next_u64()).collect()
        };
        assert_eq!(a, b);
        let c: Vec<u64> = {
            let mut r = Rng::named(s, "rewards");
            (0..5).map(|_| r.next_u64()).collect()
        };
        assert_ne!(a, c);
    }
}
