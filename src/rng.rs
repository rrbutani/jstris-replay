//! https://github.com/macmcmeans/aleaPRNG/blob/cf459e9be0d3761af923b07378fcd6ae60c42623/aleaPRNG-1.1.js

struct Mash {
    state: u32,
}

impl Mash {
    const fn new() -> Self {
        Mash {
            state: 4_022_871_197, /* 0xefc8249d */
        }
    }

    fn mash(&mut self, data: &str) -> f64 {
        for b in data.encode_utf16() {
            self.state += b as u32;

            let mut h: f64 = 0.02519603282416938 * (self.state as f64);

            self.state = h as u32;
            h -= self.state as f64;
            h *= self.state as f64;
            self.state = h as u32;
            h -= self.state as f64;

            self.state += (h * 4294967296.) as u32; // uh-oh?
        }

        self.state as f64 * 2.3283064365386963e-10
    }
}

#[cfg(test)]
mod test_mash {
    use super::*;

    #[test]
    fn asdf() {
        let mut m = Mash::new();
        assert_eq!(m.mash("asdf"), 0.9312197775579989);
    }
}

#[derive(Debug, Copy, Clone)]
pub struct AleaPrng {
    c: u32,
    s0: f64,
    s1: f64,
    s2: f64,
}

impl AleaPrng {
    pub fn new(seeds: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut masher = Mash::new();
        let mut s0 = masher.mash(" ");
        let mut s1 = masher.mash(" ");
        let mut s2 = masher.mash(" ");

        for seed in seeds {
            let seed = seed.as_ref();

            s0 -= masher.mash(seed);
            s1 -= masher.mash(seed);
            s2 -= masher.mash(seed);

            if s0 < 0. {
                s0 += 1.;
            }
            if s1 < 0. {
                s1 += 1.;
            }
            if s2 < 0. {
                s2 += 1.;
            }
        }

        Self { c: 1, s0, s1, s2 }
    }

    pub fn random(&mut self) -> f64 {
        let t = 2091639. * self.s0 + (self.c as f64) * 2.3283064365386963e-10;
        self.c = t as u32;

        self.s0 = self.s1;
        self.s1 = self.s2;
        self.s2 = t.fract();

        self.s2
    }
}

#[cfg(test)]
mod test_prng {
    use super::*;

    #[test]
    fn asdf() {
        let mut rng = dbg!(AleaPrng::new(["asdf"]));

        assert_eq!(rng.random(), 0.8024188503623009);
        assert_eq!(rng.random(), 0.4725297694094479);
        assert_eq!(rng.random(), 0.949664750834927);
        assert_eq!(rng.random(), 0.5619115477893502);
        assert_eq!(rng.random(), 0.6947485841810703);
    }
}

type OneBag = ArrayVec<Piece, { FRESH_BAG.len() }>;

pub struct JstrisBag {
    prng: AleaPrng,
    bag: OneBag,
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Piece {
    I,
    J,
    L,
    O,
    S,
    T,
    Z,
}

use arrayvec::ArrayVec;
use Piece::*;
const FRESH_BAG: [Piece; 7] = [I, O, T, L, J, S, Z];

fn fresh_bag(rng: &mut AleaPrng) -> OneBag {
    let mut bag = ArrayVec::from(FRESH_BAG);

    let array: OneBag = std::array::from_fn(|_| {
        let i = (rng.random() * bag.len() as f64).floor();
        bag.remove(i as usize)
    })
    .into();
    array.into_iter().rev().collect()
}

impl JstrisBag {
    pub fn new(seed: crate::GameSeed) -> Self {
        let mut prng = AleaPrng::new([seed]);
        let mut bag = fresh_bag(&mut prng);

        let arr = &mut *bag;
        if let Some((s_or_z, other)) = match arr {
            [.., other, S | Z, s @ (S | Z)] => Some((s, other)),
            [.., other, s @ (S | Z)] => Some((s, other)),
            _ => None,
        } {
            std::mem::swap(s_or_z, other);
        }

        Self { prng, bag }
    }

    pub fn get(&mut self) -> Piece {
        if let Some(piece) = self.bag.pop() {
            piece
        } else {
            self.bag = fresh_bag(&mut self.prng);
            self.get()
        }
    }

    pub fn iter(&mut self) -> impl Iterator<Item = Piece> + '_ {
        std::iter::from_fn(|| Some(self.get()))
    }
}
