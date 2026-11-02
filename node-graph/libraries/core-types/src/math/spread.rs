use rand::{Rng, SeedableRng};

/// Gives each of `count` items a factor from 0 to 1, either spread evenly in item order or drawn at random from the `seed`.
/// The even spread restarts after every `repeat_every` items, unless it is 0 to span all the items once.
pub fn spread_factors(count: usize, randomize: bool, seed: u64, repeat_every: u64) -> impl Iterator<Item = f64> {
	let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

	(0..count).map(move |index| match (randomize, repeat_every) {
		(true, _) => rng.random::<f64>(),
		(false, 0) => index as f64 / count.saturating_sub(1).max(1) as f64,
		(false, 1) => 0.,
		(false, _) => (index as u64 % repeat_every) as f64 / (repeat_every - 1) as f64,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn even_spread_spans_the_range_once() {
		let factors: Vec<f64> = spread_factors(5, false, 0, 0).collect();
		assert_eq!(factors, [0., 0.25, 0.5, 0.75, 1.]);

		assert_eq!(spread_factors(1, false, 0, 0).collect::<Vec<_>>(), [0.]);
		assert_eq!(spread_factors(0, false, 0, 0).count(), 0);
	}

	#[test]
	fn even_spread_repeats() {
		let factors: Vec<f64> = spread_factors(7, false, 0, 3).collect();
		assert_eq!(factors, [0., 0.5, 1., 0., 0.5, 1., 0.]);

		assert!(spread_factors(4, false, 0, 1).all(|factor| factor == 0.));
	}

	#[test]
	fn random_spread_follows_the_seed() {
		let draws = |seed| spread_factors(8, true, seed, 0).collect::<Vec<_>>();

		assert_eq!(draws(42), draws(42));
		assert_ne!(draws(42), draws(43));
		assert!(draws(42).iter().all(|factor| (0. ..1.).contains(factor)));
	}
}
