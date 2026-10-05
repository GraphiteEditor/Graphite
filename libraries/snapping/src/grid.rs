use glam::{DMat2, DVec2};

/// How nearly parallel two unit directions must be, as the sine of the angle between them, before two families count as one.
const SAME_FAMILY_SINE: f64 = 1e-6;

/// How nearly parallel to a family a line must run, as the sine of the angle between them, before it is taken never to cross that family.
const NEVER_CROSSING_SINE: f64 = 1e-12;

/// One set of parallel, evenly spaced grid lines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineFamily {
	/// The unit direction the lines run along.
	pub direction: DVec2,
	/// The distance between neighboring lines, measured straight across them.
	pub spacing: f64,
}

impl LineFamily {
	/// The unit direction straight across the lines.
	fn across(&self) -> DVec2 {
		self.direction.perp()
	}
}

/// Families of evenly spaced lines sharing an origin. A feature lands where the first two families meet while grid snapping is on.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
	origin: DVec2,
	families: Vec<LineFamily>,
	/// The steps from one grid point to its neighbors, as the columns.
	steps: DMat2,
}

impl Grid {
	/// Returns `None` unless there are at least two families, no two of them parallel, each with a positive and finite spacing.
	pub fn new(origin: DVec2, families: impl IntoIterator<Item = (DVec2, f64)>) -> Option<Self> {
		let families = families
			.into_iter()
			.map(|(direction, spacing)| {
				Some(LineFamily {
					direction: direction.try_normalize()?,
					spacing,
				})
			})
			.collect::<Option<Vec<_>>>()?;

		let spaced = families.iter().all(|family| family.spacing.is_finite() && family.spacing > 0.);
		let distinct = families
			.iter()
			.enumerate()
			.all(|(index, family)| families[..index].iter().all(|earlier| family.direction.perp_dot(earlier.direction).abs() > SAME_FAMILY_SINE));
		if !origin.is_finite() || families.len() < 2 || !spaced || !distinct {
			return None;
		}

		// Stepping one spacing across either of the first two families, while staying on a line of the other, reaches a neighboring point
		let [first, second] = [families[0], families[1]];
		let across_to_plane = DMat2::from_cols(first.across(), second.across()).transpose().inverse();
		let steps = across_to_plane * DMat2::from_diagonal(DVec2::new(first.spacing, second.spacing));

		steps.is_finite().then_some(Self { origin, families, steps })
	}

	/// A grid of vertical lines `spacing.x` apart and horizontal lines `spacing.y` apart.
	pub fn rectangular(origin: DVec2, spacing: DVec2) -> Option<Self> {
		Self::new(origin, [(DVec2::Y, spacing.x), (DVec2::X, spacing.y)])
	}

	pub fn families(&self) -> &[LineFamily] {
		&self.families
	}

	/// How many spacings of a family a position lies from the family's line through the origin.
	fn steps_across(&self, family: &LineFamily, position: DVec2) -> f64 {
		(position - self.origin).dot(family.across()) / family.spacing
	}

	/// The grid point closest to a position.
	pub fn nearest_point(&self, position: DVec2) -> DVec2 {
		self.nearest_point_as_seen(position, DMat2::IDENTITY)
	}

	/// The grid point that appears closest to a position, when `seen` carries small steps in the plane to steps on screen.
	pub fn nearest_point_as_seen(&self, position: DVec2, seen: DMat2) -> DVec2 {
		let seen = if seen.is_finite() && seen.determinant() != 0. { seen } else { DMat2::IDENTITY };
		let appears = |step: DVec2| (seen * step).length_squared();

		// Shortening each step by whole multiples of the other leaves the same points, with the closest now among those around the position
		let (mut shorter, mut longer) = (self.steps.x_axis, self.steps.y_axis);
		for _ in 0..64 {
			if appears(longer) < appears(shorter) {
				std::mem::swap(&mut shorter, &mut longer);
			}

			let multiple = ((seen * longer).dot(seen * shorter) / appears(shorter)).round();
			if multiple == 0. || !multiple.is_finite() {
				break;
			}
			longer -= shorter * multiple;
		}

		let steps = DMat2::from_cols(shorter, longer);
		let rounded = (steps.inverse() * (position - self.origin)).round();
		let around = (-1..=1).flat_map(|first| (-1..=1).map(move |second| DVec2::new(first as f64, second as f64)));
		let points = around.map(|offset| self.origin + steps * (rounded + offset));

		points.min_by(|&a, &b| appears(a - position).total_cmp(&appears(b - position))).unwrap_or(position)
	}

	/// For each family, the places where its lines on either side of `near` cross the endless line through `point` along `direction`.
	/// A family the line runs along gives none, which leaves nothing at all for a direction of no length.
	pub fn crossings_along(&self, point: DVec2, direction: DVec2, near: DVec2) -> Vec<DVec2> {
		let Some(direction) = direction.try_normalize() else { return Vec::new() };
		let mut crossings = Vec::new();

		for family in &self.families {
			let progress_across = direction.dot(family.across());
			if progress_across.abs() < NEVER_CROSSING_SINE {
				continue;
			}

			let steps = self.steps_across(family, near);
			for line in [steps.floor(), steps.ceil()] {
				let along = (line - self.steps_across(family, point)) * family.spacing / progress_across;
				let crossing = point + direction * along;

				if crossing.is_finite() {
					crossings.push(crossing);
				}
			}
		}

		crossings
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::f64::consts::PI;

	/// The grid point that appears closest to a position, found the slow way by trying every point for a long way around.
	fn nearest_by_trying_every_point(grid: &Grid, position: DVec2, seen: DMat2) -> DVec2 {
		let whole_steps = (grid.steps.inverse() * (position - grid.origin)).round();
		let appears = |point: DVec2| (seen * (point - position)).length_squared();

		let mut nearest = grid.origin + grid.steps * whole_steps;
		for first in -60..=60 {
			for second in -60..=60 {
				let point = grid.origin + grid.steps * (whole_steps + DVec2::new(first as f64, second as f64));
				if appears(point) < appears(nearest) {
					nearest = point;
				}
			}
		}
		nearest
	}

	#[test]
	fn unusable_grids_are_rejected() {
		assert!(Grid::rectangular(DVec2::ZERO, DVec2::new(10., 0.)).is_none());
		assert!(Grid::rectangular(DVec2::ZERO, DVec2::new(10., f64::NAN)).is_none());
		assert!(Grid::new(DVec2::ZERO, [(DVec2::X, 1.)]).is_none());
		assert!(Grid::new(DVec2::ZERO, [(DVec2::X, 1.), (DVec2::NEG_X, 2.)]).is_none());
		assert!(Grid::new(DVec2::ZERO, [(DVec2::X, 1.), (DVec2::from_angle(1e-8), 1.)]).is_none());
	}

	#[test]
	fn the_nearest_point_of_a_rectangular_grid_rounds_each_direction_from_the_origin() {
		let grid = Grid::rectangular(DVec2::new(1., 2.), DVec2::new(10., 5.)).unwrap();

		assert!(grid.nearest_point(DVec2::new(14., 9.)).abs_diff_eq(DVec2::new(11., 7.), 1e-9));
		assert!(grid.nearest_point(DVec2::new(17., 10.)).abs_diff_eq(DVec2::new(21., 12.), 1e-9));
		assert!(grid.nearest_point(DVec2::new(-3., -4.)).abs_diff_eq(DVec2::new(1., -3.), 1e-9));
		assert!(grid.nearest_point(DVec2::new(11., 7.)).abs_diff_eq(DVec2::new(11., 7.), 1e-9));
	}

	#[test]
	fn the_nearest_point_of_an_isometric_grid_is_nearer_than_every_neighbor() {
		let directions = [PI / 2., PI / 6., 5. * PI / 6.].map(DVec2::from_angle);
		let grid = Grid::new(DVec2::ZERO, directions.map(|direction| (direction, 10.))).unwrap();

		// Stepping by one spacing across the first two families from a grid point reaches its neighbors
		let neighbors = [(1., 0.), (0., 1.), (1., 1.), (-1., 0.), (0., -1.), (-1., -1.), (1., -1.), (-1., 1.)];

		for step in 0..500 {
			let position = DVec2::new((step as f64 * 0.731).sin() * 60., (step as f64 * 1.37).cos() * 60.);
			let nearest = grid.nearest_point(position);

			// A grid point sits on a line of every family
			for family in grid.families() {
				let steps = grid.steps_across(family, nearest);
				assert!((steps - steps.round()).abs() < 1e-9);
			}

			for (first, second) in neighbors {
				let offset = DMat2::from_cols(directions[0].perp(), directions[1].perp()).transpose().inverse() * DVec2::new(first * 10., second * 10.);
				assert!(nearest.distance(position) <= (nearest + offset).distance(position) + 1e-9);
			}
		}
	}

	#[test]
	fn the_nearest_point_is_found_however_slanted_the_grid_or_the_view() {
		// Families only a few degrees apart make cells so slanted that the closest point is far from the corners of the cell around a position
		let slanted = Grid::new(DVec2::new(3., -2.), [(DVec2::from_angle(0.3), 10.), (DVec2::from_angle(0.38), 7.)]).unwrap();
		let square = Grid::rectangular(DVec2::ZERO, DVec2::splat(10.)).unwrap();

		// A view that shears and stretches makes a square grid appear just as slanted
		let views = [DMat2::IDENTITY, DMat2::from_cols(DVec2::new(1., 0.), DVec2::new(4.3, 0.4))];

		for grid in [slanted, square] {
			for seen in views {
				for step in 0..200 {
					let position = DVec2::new((step as f64 * 0.731).sin() * 90., (step as f64 * 1.37).cos() * 90.);

					let nearest = grid.nearest_point_as_seen(position, seen);
					let expected = nearest_by_trying_every_point(&grid, position, seen);
					assert!(nearest.abs_diff_eq(expected, 1e-6), "{nearest:?} should be {expected:?} for {position:?}");
				}
			}
		}
	}

	#[test]
	fn a_line_is_crossed_by_the_grid_lines_on_either_side_of_a_position() {
		let grid = Grid::rectangular(DVec2::ZERO, DVec2::new(10., 10.)).unwrap();
		let contains = |crossings: &[DVec2], expected: DVec2| crossings.iter().any(|crossing| crossing.abs_diff_eq(expected, 1e-9));

		// A horizontal line off the grid only ever crosses the vertical lines
		let crossings = grid.crossings_along(DVec2::new(0., 3.), DVec2::X * 5., DVec2::new(17., 3.));
		assert_eq!(crossings.len(), 2);
		assert!(contains(&crossings, DVec2::new(10., 3.)) && contains(&crossings, DVec2::new(20., 3.)));

		// A diagonal crosses both families
		let crossings = grid.crossings_along(DVec2::new(1., 0.), DVec2::ONE, DVec2::new(9.5, 8.5));
		assert_eq!(crossings.len(), 4);
		assert!(contains(&crossings, DVec2::new(0., -1.)) && contains(&crossings, DVec2::new(10., 9.)));
		assert!(contains(&crossings, DVec2::new(1., 0.)) && contains(&crossings, DVec2::new(11., 10.)));

		assert!(grid.crossings_along(DVec2::ZERO, DVec2::ZERO, DVec2::ONE).is_empty());
	}
}
