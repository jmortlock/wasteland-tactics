//! Combat arithmetic. Pure functions plus the single seeded RNG resource.

use rand::Rng;
#[cfg(test)]
use rand::SeedableRng;
#[cfg(test)]
use rand_chacha::ChaCha8Rng;

/// The one RNG the simulation uses, so a battle replays identically for a given seed.
#[cfg(test)]
#[derive(bevy::prelude::Resource)]
pub struct GameRng(pub ChaCha8Rng);

#[cfg(test)]
impl GameRng {
    pub fn seeded(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }
}

/// Probability (0..=1) of a shot landing. 0.9 point blank, −0.04 per cell, floor 0.1, halved by cover.
pub fn hit_chance(distance: i32, range: i32, in_cover: bool) -> f32 {
    if distance < 0 || distance > range {
        return 0.0;
    }
    let base = (0.9 - 0.04 * distance as f32).clamp(0.1, 0.95);
    if in_cover { base * 0.5 } else { base }
}

pub fn roll_hit(rng: &mut impl Rng, chance: f32) -> bool {
    chance > 0.0 && rng.random::<f32>() < chance
}

pub fn roll_damage(rng: &mut impl Rng, min: i32, max: i32) -> i32 {
    rng.random_range(min..=max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_chance_falls_with_distance_and_is_zero_out_of_range() {
        assert_eq!(hit_chance(0, 8, false), 0.9);
        assert!(hit_chance(4, 8, false) < hit_chance(1, 8, false));
        assert_eq!(hit_chance(9, 8, false), 0.0);
        assert!(
            hit_chance(8, 8, false) >= 0.1,
            "never below the floor while in range"
        );
    }

    #[test]
    fn cover_halves_hit_chance() {
        assert_eq!(hit_chance(2, 8, true), hit_chance(2, 8, false) * 0.5);
    }

    #[test]
    fn seeded_rng_is_deterministic() {
        let mut a = GameRng::seeded(42);
        let mut b = GameRng::seeded(42);
        let ra: Vec<bool> = (0..20).map(|_| roll_hit(&mut a.0, 0.5)).collect();
        let rb: Vec<bool> = (0..20).map(|_| roll_hit(&mut b.0, 0.5)).collect();
        assert_eq!(ra, rb);
        assert!(
            ra.iter().any(|h| *h) && ra.iter().any(|h| !*h),
            "not degenerate"
        );
    }

    #[test]
    fn damage_is_within_bounds() {
        let mut rng = GameRng::seeded(1);
        for _ in 0..200 {
            let d = roll_damage(&mut rng.0, 10, 25);
            assert!((10..=25).contains(&d));
        }
        assert!(!roll_hit(&mut rng.0, 0.0));
        assert!(roll_hit(&mut rng.0, 1.0));
    }
}
