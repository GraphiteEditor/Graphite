//! The kernel rendering Mandelbrot-style fractals, which the GPU pass mirrors in `vello_generators_mandelbrot.wgsl`.
//! Points follow a reference orbit iterated in 64 bits by their small offset from it in 32 bits (perturbation), which keeps deep zooms precise.
//! With a power other than an integer, they do so only where 32-bit floats can't place them to iterate them directly.

use glam::{DVec2, Vec2};
use graphic_types::generator::{MandelbrotExterior, MandelbrotFractal, MandelbrotInterior, MandelbrotSettings};
use std::f32::consts::TAU;
use std::f64::consts::FRAC_PI_2;

/// The most iterations a point can be given, beyond which it counts as inside.
pub const MAX_ITERATIONS: u32 = 10_000;

/// The lowest and highest powers iterated, which lower and higher powers are clamped to. Below the lowest, the set's escape radius of 2^(1 / (power - 1))
/// grows too large for 32-bit floats to square, and above the highest, the set is little more than a disk.
pub const POWER_RANGE: (f64, f64) = (1.02, 32.);

/// How many of a 32-bit float's steps one pixel must span for points with a power other than an integer to be iterated directly, past which they follow a reference orbit.
const MIN_DIRECT_PIXEL_STEPS: f64 = 64.;

/// How far an offset can reach from the reference value, relative to it, for a power other than an integer to step the offset by itself rather than raise its value directly.
const OFFSET_STEP_REACH: f32 = 0.5;

/// How large a number can be for the logarithm of one plus it, and its exponential less one, to be summed as series, where 32-bit functions would lose its precision.
const SERIES_REACH: f32 = 0.125;

/// The terms of those series, which keep them within 32-bit rounding up to their reach.
const LOGARITHM_TERMS: u32 = 7;
const EXPONENTIAL_TERMS: u32 = 5;

/// The candidates across and down an area that its reference orbit is chosen from, as the one escaping last.
const REFERENCE_CANDIDATES: usize = 5;

/// The points across and down the plane that the scan for a thumbnail's area samples, an odd number so a row and column run through the slice's center,
/// where the Mandelbrot set's thin antenna lies along the real axis.
const THUMBNAIL_SCAN_POINTS: usize = 65;

/// How many iterations a point the scan for a thumbnail's area samples must last to count as part of the fractal's body.
const THUMBNAIL_SCAN_ITERATIONS: u32 = 32;

/// The largest escape radius the scan for a thumbnail's area spans, which low powers' far larger sets are cut down to.
const THUMBNAIL_SCAN_RADIUS: f64 = 16.;

/// How much of its size the area a thumbnail frames grows by on each side, past the fractal's body.
const THUMBNAIL_MARGIN: f64 = 0.1;

/// How close a settled orbit's value must come back to where it was to count as one turn of its cycle, however close its closest return.
const PERIOD_TOLERANCE: f32 = 1e-5;

/// How close a settled orbit's value must come back to where it was at least once for it to count as cycling at all.
const SETTLED_TOLERANCE: f32 = 1e-2;

/// How close a value must come to where it lands a turn of its cycle later for the orbit to count as settled into the cycle, which sets the Discrete Settling's bands.
const SETTLING_TOLERANCE: f32 = 1e-3;

/// How many doublings of a point's distance to the set span the whole gradient, from 1 unit to 2^-16 units.
const DISTANCE_DOUBLINGS: f32 = 16.;

/// Whether 32-bit floats place pixels `texel` apart across `area` of the plane precisely, with each pixel spanning enough of their steps.
pub fn resolves_in_32_bits(area: [DVec2; 2], texel: f64) -> bool {
	let farthest = area[0].abs().max(area[1].abs()).max_element();
	texel >= farthest * f32::EPSILON as f64 * MIN_DIRECT_PIXEL_STEPS
}

/// The settings in the form the kernel iterates with, which the GPU pass takes too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MandelbrotSetup {
	/// How many units of the pattern's space span one unit of the complex plane.
	pub scale: f64,
	pub fractal: MandelbrotFractal,
	pub power: f64,
	/// The power as an integer, if it is one, for which points always follow a reference orbit.
	pub integer_power: Option<u32>,
	pub iterations: u32,
	/// How much the starting value, and the point added each iteration, move with a point of the plane.
	/// As the slice rotates from the fractal's set toward the Julia set, these turn from 0 and 1 to 1 and 0 with a steady scale, since their squares sum to 1.
	pub start_slope: f64,
	pub added_slope: f64,
	/// The starting value, and the point added each iteration, at the plane's origin.
	pub start_constant: DVec2,
	pub added_constant: DVec2,
	pub exterior: MandelbrotExterior,
	pub frequency: f32,
	pub interior: MandelbrotInterior,
	/// The radius past which a point counts as escaped.
	pub bailout: f64,
}

/// An orbit iterated in 64 bits from a reference point, which points near it follow by their small offset from it in 32 bits.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceOrbit {
	/// The reference point in the plane the fractal is sampled in.
	pub point: DVec2,
	/// The orbit's values while it stays within the bailout radius, from its starting value on.
	pub values: Vec<Vec2>,
	/// Each value as the formula raises it to the power, from which points step with a power other than an integer.
	pub raised_values: Vec<Vec2>,
}

impl MandelbrotSetup {
	pub fn new(settings: &MandelbrotSettings) -> Self {
		let power = settings.power.clamp(POWER_RANGE.0, POWER_RANGE.1);
		let integer_power = (power.fract() == 0.).then_some(power as u32);

		// Past the set's own escape radius, and large enough for the power to swamp the point added each iteration, which smooths the escape counts,
		// but small enough that an escaped value's squared length, and the derivative's growth each iteration, stay well within 32 bits
		let bailout = (2. * 2_f64.powf(1. / (power - 1.))).max(256_f64.min(2_f64.powf(30. / power)));

		// A quarter turn spans Mandelbrot-to-Julia, ending exactly on the Julia set
		let mandelbrot_to_julia = (settings.mandelbrot_to_julia / 100.).clamp(0., 1.);
		let (start_slope, added_slope) = if mandelbrot_to_julia < 1. { (mandelbrot_to_julia * FRAC_PI_2).sin_cos() } else { (1., 0.) };

		Self {
			scale: settings.scale.max(f64::MIN_POSITIVE),
			fractal: settings.fractal,
			power,
			integer_power,
			iterations: settings.iterations.clamp(1, MAX_ITERATIONS as i64) as u32,
			start_slope,
			added_slope,
			start_constant: settings.mandelbrot_slice * added_slope,
			added_constant: settings.julia_slice * start_slope,
			exterior: settings.exterior,
			frequency: settings.frequency.clamp(1, 64) as f32,
			interior: settings.interior,
			bailout,
		}
	}

	/// Where a point of the pattern's space lies in the complex plane.
	pub fn plane_point(&self, point: DVec2) -> DVec2 {
		point / self.scale
	}

	/// The starting value and the point added each iteration for a point of the plane, which turn from the fractal's set's to the Julia set's as the slice rotates.
	fn start(&self, point: DVec2) -> (DVec2, DVec2) {
		(point * self.start_slope + self.start_constant, point * self.added_slope + self.added_constant)
	}

	/// The orbit for points `texel` apart across `area` of the plane to follow. A power other than an integer has none where 32-bit floats place those points
	/// precisely enough to iterate them directly, which spares raising its candidates to the power in 64 bits.
	pub fn reference_orbit(&self, area: [DVec2; 2], texel: f64) -> Option<ReferenceOrbit> {
		if self.integer_power.is_none() && resolves_in_32_bits(area, texel) {
			return None;
		}
		self.reference_orbit_across(area)
	}

	/// The orbit escaping last among candidates spread across `area` of the plane.
	fn reference_orbit_across(&self, area: [DVec2; 2]) -> Option<ReferenceOrbit> {
		// Visits each of a point's orbit values within the bailout radius, with the formula's raising of it, in 64 bits
		let orbit = |point: DVec2, visit: &mut dyn FnMut(DVec2, DVec2)| {
			let (mut z, c) = self.start(point);
			for _ in 0..self.iterations {
				if z.length_squared() > self.bailout * self.bailout {
					break;
				}
				let raised = self.formula_in_64_bits(z);
				visit(z, raised);
				z = raised + c;
			}
		};
		let length = |point: DVec2| {
			let mut length = 0;
			orbit(point, &mut |_, _| length += 1);
			length
		};

		// O(candidates × iterations)
		let candidates = (0..REFERENCE_CANDIDATES * REFERENCE_CANDIDATES).map(|index| {
			let fraction = (DVec2::new((index % REFERENCE_CANDIDATES) as f64, (index / REFERENCE_CANDIDATES) as f64) + 0.5) / REFERENCE_CANDIDATES as f64;
			area[0] + (area[1] - area[0]) * fraction
		});
		let point = candidates.max_by_key(|&point| length(point))?;

		let (mut values, mut raised_values) = (Vec::new(), Vec::new());
		orbit(point, &mut |z, raised| {
			values.push(z.as_vec2());
			raised_values.push(raised.as_vec2());
		});
		Some(ReferenceOrbit { point, values, raised_values })
	}

	/// The area of the pattern's own space that a thumbnail frames: where points of the plane last the scan's iterations without escaping, with a margin.
	/// This finds the fractal's body for any slice, power, or formula, including Julia sets of dust that every point eventually escapes.
	/// O(scan points² × scan iterations).
	pub fn thumbnail_area(&self) -> [DVec2; 2] {
		// Every value past the escape radius escapes, so the body lies within that radius of the slice's offsets, over the larger of its slopes
		let escape_radius = 2_f64.powf(1. / (self.power - 1.)).min(THUMBNAIL_SCAN_RADIUS);
		let reach = (escape_radius + self.start_constant.length() + self.added_constant.length()) / self.start_slope.max(self.added_slope);

		// The bounds of the scanned points lasting the scan's iterations
		let lasts = |point: DVec2| {
			let (mut z, c) = self.start(point);
			(0..THUMBNAIL_SCAN_ITERATIONS).all(|_| {
				z = self.formula_in_64_bits(z) + c;
				z.length_squared() <= escape_radius * escape_radius
			})
		};
		let spacing = 2. * reach / (THUMBNAIL_SCAN_POINTS - 1) as f64;
		let body = (0..THUMBNAIL_SCAN_POINTS * THUMBNAIL_SCAN_POINTS)
			.map(|index| DVec2::new((index % THUMBNAIL_SCAN_POINTS) as f64, (index / THUMBNAIL_SCAN_POINTS) as f64) * spacing - reach)
			.filter(|&point| lasts(point))
			.fold(None, |bounds: Option<[DVec2; 2]>, point| Some(bounds.map_or([point; 2], |[min, max]| [min.min(point), max.max(point)])));

		// The body, grown by the margin and the spacing between the scanned points it lies among, or the whole scan if no point lasts
		let [min, max] = body.map_or([DVec2::splat(-reach), DVec2::splat(reach)], |[min, max]| {
			let margin = (max - min) * THUMBNAIL_MARGIN + spacing;
			[min - margin, max + margin]
		});
		[min * self.scale, max * self.scale]
	}

	/// Whether the main cardioid and the bulb on its left are inside the set exactly where a test of a point's position says, as they are for the Mandelbrot set itself.
	pub fn has_interior_test(&self) -> bool {
		self.fractal == MandelbrotFractal::ClassicMandelbrot && self.integer_power == Some(2) && self.start_slope == 0. && self.start_constant == DVec2::ZERO
	}

	/// The formula's next value before the point is added, in 64 bits.
	fn formula_in_64_bits(&self, z: DVec2) -> DVec2 {
		let z = match self.fractal {
			MandelbrotFractal::ClassicMandelbrot => z,
			MandelbrotFractal::BurningShip => z.abs(),
			MandelbrotFractal::Tricorn => DVec2::new(z.x, -z.y),
		};
		complex_power_64(z, self.power)
	}

	/// The fractal's brightness at `point` of the plane, from 0 to 1.
	/// Points follow `reference` where there is one, and are otherwise iterated directly in 64 bits.
	/// With `interior_test`, points tested to lie inside the main cardioid or the bulb on its left skip their iterations.
	pub fn sample(&self, point: DVec2, reference: Option<&ReferenceOrbit>, interior_test: bool) -> f32 {
		if interior_test && let Some(brightness) = self.known_interior(point.as_vec2()) {
			return brightness;
		}

		match reference {
			Some(reference) if reference.values.len() >= 2 => {
				let offset = point - reference.point;
				let orbit = PerturbedOrbit {
					setup: self,
					reference,
					index: 0,
					offset: (offset * self.start_slope).as_vec2(),
					added_offset: (offset * self.added_slope).as_vec2(),
				};
				self.brightness(orbit)
			}
			_ => {
				let (z, c) = self.start(point);
				self.brightness(DirectOrbit { setup: self, z, c })
			}
		}
	}

	/// The brightness of a point the interior test finds inside the main cardioid or the bulb on its left, for the interiors that don't need its iterations.
	fn known_interior(&self, point: Vec2) -> Option<f32> {
		let needs_iterations = !matches!(
			self.interior,
			MandelbrotInterior::Flat | MandelbrotInterior::Period | MandelbrotInterior::Radius | MandelbrotInterior::Angle | MandelbrotInterior::Rays
		);
		if needs_iterations {
			return None;
		}
		let period = cardioid_or_bulb_period(point.x, point.y)?;

		// The cycle's multiplier is 1 - √(1 - 4c) across the cardioid, and 4(c + 1) across the bulb
		let multiplier = match period {
			1 => Vec2::X - complex_square_root(Vec2::X - point * 4.),
			_ => (point + Vec2::X) * 4.,
		};
		Some(self.cycle_brightness(Some((period, multiplier))))
	}

	/// The brightness that a point's orbit shows, iterating it until it escapes, cycles, or runs out of iterations.
	fn brightness<O: Orbit + Clone>(&self, mut orbit: O) -> f32 {
		let bailout_squared = (self.bailout * self.bailout) as f32;

		// Settling compares the orbit with itself from the start again, so only that interior keeps its start
		let start = (self.interior == MandelbrotInterior::DiscreteSettling).then(|| orbit.clone());
		let records = matches!(self.exterior, MandelbrotExterior::ClosestApproach | MandelbrotExterior::ClosestIteration)
			|| matches!(
				self.interior,
				MandelbrotInterior::ClosestApproach | MandelbrotInterior::ClosestIteration | MandelbrotInterior::SmoothSettling
			);

		// The orbit's derivative with respect to the point, which starts at how much the starting value moves with it
		let mut derivative = Vec2::new(self.start_slope as f32, 0.);
		let mut stripes = Stripes::default();
		let mut escape_angle = EscapeAngle::default();
		let mut record = OrbitRecord::default();

		for iteration in 1..=self.iterations {
			if self.exterior == MandelbrotExterior::Proximity {
				derivative = self.derivative_step(orbit.value(), derivative);
			}

			orbit.step();
			let z = orbit.value();

			if self.exterior == MandelbrotExterior::Tendrils {
				stripes.visit(z, self.frequency);
			}

			if self.exterior == MandelbrotExterior::Angle {
				escape_angle.visit(iteration, z, self.fractal, self.power as f32);
			}

			if z.length_squared() > bailout_squared {
				return self.exterior(iteration, z, derivative, stripes, escape_angle, record);
			}

			if records {
				record.visit(iteration, z, self.power as f32);
			}
		}

		self.interior(orbit, start, record)
	}

	/// The derivative of the orbit's next value with respect to the point, given its current value `z` and derivative.
	fn derivative_step(&self, z: Vec2, derivative: Vec2) -> Vec2 {
		self.chain_step(z, derivative) + Vec2::new(self.added_slope as f32, 0.)
	}

	/// The formula's derivative at the value `z` times `derivative`, chaining the derivative of the value before it through one iteration.
	fn chain_step(&self, z: Vec2, derivative: Vec2) -> Vec2 {
		let power = self.power as f32;
		let (z, derivative) = match self.fractal {
			MandelbrotFractal::ClassicMandelbrot => (z, derivative),
			MandelbrotFractal::BurningShip => (z.abs(), derivative * Vec2::new(z.x.signum(), z.y.signum())),
			MandelbrotFractal::Tricorn => (conjugate(z), conjugate(derivative)),
		};
		complex_multiply(complex_power(z, power - 1.) * power, derivative)
	}

	/// The brightness of a point that escaped on `iteration` with the value `z`.
	fn exterior(&self, iteration: u32, z: Vec2, derivative: Vec2, stripes: Stripes, escape_angle: EscapeAngle, record: OrbitRecord) -> f32 {
		let power = self.power as f32;

		// How far past the bailout radius the value landed, from 0 just past it to 1 at the bailout radius raised to the power
		let log_magnitude = 0.5 * z.length_squared().ln();
		let overshoot = ((log_magnitude / self.bailout.ln() as f32).ln() / power.ln()).clamp(0., 1.);
		let escape = (iteration as f32 - overshoot).max(0.);

		match self.exterior {
			MandelbrotExterior::Flat => 0.,
			MandelbrotExterior::SmoothEscape => (escape + 1.).ln() / (self.iterations as f32 + 1.).ln(),
			// The smooth escape at the start of its band, where the value lands farthest past the bailout radius
			MandelbrotExterior::DiscreteEscape => (iteration as f32).ln() / (self.iterations as f32 + 1.).ln(),
			MandelbrotExterior::Proximity => {
				let distance = 2. * log_magnitude.exp() * log_magnitude / length_without_overflow(derivative);
				if distance > 0. && distance.is_finite() {
					(-distance.log2() / DISTANCE_DOUBLINGS).max(0.)
				} else {
					1.
				}
			}
			MandelbrotExterior::Angle => (escape_angle.radians / TAU + 0.5).rem_euclid(1.),
			MandelbrotExterior::ClosestApproach => record.closest_approach(),
			MandelbrotExterior::ClosestIteration => record.closest_iteration_brightness(),
			// Blended between the averages with and without the last value, so the stripes run on unbroken where points escape one iteration later
			MandelbrotExterior::Tendrils => {
				let average = stripes.sum / iteration as f32;
				let previous = if iteration > 1 { (stripes.sum - stripes.last) / (iteration - 1) as f32 } else { average };
				previous + (average - previous) * (1. - overshoot)
			}
			// The angle scales by the power across the overshoot, so the lines run on unbroken where points escape one iteration later.
			// The Burning Ship folds the value into a quarter turn before raising it to the power, so its lines take that angle at twice the frequency to join up the same way.
			MandelbrotExterior::Rays => {
				let (angle, frequency) = match self.fractal {
					MandelbrotFractal::BurningShip => (z.y.abs().atan2(z.x.abs()), 2. * self.frequency),
					MandelbrotFractal::ClassicMandelbrot | MandelbrotFractal::Tricorn => (z.y.atan2(z.x), self.frequency),
				};
				0.5 * (frequency * power.powf(1. - overshoot) * angle).cos() + 0.5
			}
		}
	}

	/// The brightness of a point inside the set, given its orbit after all its iterations, the orbit's `start` if the interior keeps it, and what it recorded along the way.
	fn interior<O: Orbit + Clone>(&self, orbit: O, start: Option<O>, record: OrbitRecord) -> f32 {
		let value = match self.interior {
			MandelbrotInterior::Flat => return 1.,
			MandelbrotInterior::Period | MandelbrotInterior::Radius | MandelbrotInterior::Angle | MandelbrotInterior::Rays => return self.cycle_brightness(self.settled_cycle(orbit)),
			// Brightest for the orbits that settle last, at the start of the band they settle in, like the discrete escape outside
			MandelbrotInterior::DiscreteSettling => {
				let settling = start.zip(self.settled_cycle(orbit)).and_then(|(start, (period, _))| self.settling_iteration(start, period));
				settling.map_or(1., |iteration| (iteration as f32 + 1.).ln() / (self.iterations as f32 + 1.).ln())
			}
			MandelbrotInterior::Proximity => self.interior_edge_distance(orbit),
			MandelbrotInterior::FinalDistance => orbit.value().length() / 2.,
			MandelbrotInterior::ClosestApproach => record.closest_approach(),
			MandelbrotInterior::ClosestIteration => record.closest_iteration_brightness(),
			MandelbrotInterior::FinalAngle => {
				let z = orbit.value();
				z.y.atan2(z.x) / TAU + 0.5
			}
			// The geometric mean of how much each iteration scales distances, which is the exponential of the Lyapunov exponent
			MandelbrotInterior::SmoothSettling => (record.log_derivative_sum / self.iterations as f32).exp(),
		};
		value.min(1.)
	}

	/// The length and multiplier of the cycle an orbit has settled into after all its iterations, if it has.
	/// The length is the first number of iterations bringing the value back about as close as any does, and the multiplier is the derivative of that many iterations.
	fn settled_cycle<O: Orbit + Clone>(&self, mut orbit: O) -> Option<(u32, Vec2)> {
		// Comparing with the closest return finds the cycles of orbits still settling near a bulb's edge, which no fixed tolerance would
		let settled = orbit.value();
		let mut returning = orbit.clone();
		let distance = |orbit: &mut O| {
			orbit.step();
			(orbit.value() - settled).length_squared()
		};
		let closest = (0..self.iterations).map(|_| distance(&mut returning)).fold(f32::INFINITY, f32::min);
		if closest >= SETTLED_TOLERANCE * SETTLED_TOLERANCE {
			return None;
		}

		let mut multiplier = Vec2::X;
		for period in 1..=self.iterations {
			multiplier = self.chain_step(orbit.value(), multiplier);
			if distance(&mut orbit) <= 4. * closest + PERIOD_TOLERANCE * PERIOD_TOLERANCE {
				return Some((period, multiplier));
			}
		}
		None
	}

	/// The first iteration from which the orbit, from its `start`, repeats itself each turn of its `period`-long cycle to within the settling tolerance.
	/// O(iterations), stepping the orbit twice over.
	fn settling_iteration<O: Orbit + Clone>(&self, start: O, period: u32) -> Option<u32> {
		let mut following = start.clone();
		let mut leading = start;
		for _ in 0..period {
			leading.step();
		}

		(0..self.iterations).find(|_| {
			let settled = (leading.value() - following.value()).length_squared() < SETTLING_TOLERANCE * SETTLING_TOLERANCE;
			leading.step();
			following.step();
			settled
		})
	}

	/// How close a point inside the set lies to its bulb's edge, from how the multiplier of the cycle its orbit settles into changes with the point.
	/// This is the interior distance estimate, mapped to brightness like the edge distance outside.
	fn interior_edge_distance<O: Orbit + Clone>(&self, mut cycle: O) -> f32 {
		let Some((period, _)) = self.settled_cycle(cycle.clone()) else { return 1. };
		let power = self.power as f32;

		// Around the cycle, the derivatives of its iterations by the value and by the point added each iteration, and the second derivatives by the value twice and by both
		let (mut by_value, mut by_added, mut by_value_twice, mut by_both) = (Vec2::X, Vec2::ZERO, Vec2::ZERO, Vec2::ZERO);
		for _ in 0..period {
			let z = self.formula_input(cycle.value());
			let first = complex_power(z, power - 1.) * power;
			let second = complex_power(z, power - 2.) * (power * (power - 1.));

			by_both = complex_multiply(second, complex_multiply(by_value, by_added)) + complex_multiply(first, by_both);
			by_value_twice = complex_multiply(second, complex_multiply(by_value, by_value)) + complex_multiply(first, by_value_twice);
			by_added = complex_multiply(first, by_added) + Vec2::X;
			by_value = complex_multiply(first, by_value);
			cycle.step();
		}

		// The multiplier changes with the point by the mixed derivative plus the second derivative times how far the cycle shifts,
		// and the edge lies about as far away as the multiplier's size has left to reach 1, divided by that rate
		let shift = complex_divide(by_added, Vec2::X - by_value);
		let rate = length_without_overflow(by_both + complex_multiply(by_value_twice, shift)) * self.added_slope as f32;
		let distance = ((1. - by_value.length_squared()) / rate).max(0.);
		if distance.is_finite() { (-distance.log2() / DISTANCE_DOUBLINGS).clamp(0., 1.) } else { 0. }
	}

	/// What the formula does to the value `z` before raising it to the power.
	fn formula_input(&self, z: Vec2) -> Vec2 {
		match self.fractal {
			MandelbrotFractal::ClassicMandelbrot => z,
			MandelbrotFractal::BurningShip => z.abs(),
			MandelbrotFractal::Tricorn => conjugate(z),
		}
	}

	/// The brightness inside the set from the length and multiplier of the cycle a point's orbit settles into, if one was found.
	fn cycle_brightness(&self, cycle: Option<(u32, Vec2)>) -> f32 {
		match self.interior {
			MandelbrotInterior::Radius => cycle.map_or(1., |(_, multiplier)| multiplier.length().min(1.)),
			MandelbrotInterior::Angle => cycle.map_or(1., |(_, multiplier)| multiplier.y.atan2(multiplier.x) / TAU + 0.5),
			MandelbrotInterior::Rays => cycle.map_or(1., |(_, multiplier)| 0.5 * (self.frequency * multiplier.y.atan2(multiplier.x)).cos() + 0.5),
			MandelbrotInterior::Period => cycle.map_or(1., |(period, _)| 1. - 1. / period as f32),
			_ => 1.,
		}
	}
}

/// The stripes a point's orbit draws for the Tendrils exterior, from the direction each of its values points in.
#[derive(Clone, Copy, Default)]
struct Stripes {
	sum: f32,
	last: f32,
}

impl Stripes {
	/// Adds the stripe of the value `z`, with `frequency` stripes for each turn.
	fn visit(&mut self, z: Vec2, frequency: f32) {
		self.last = 0.5 * (frequency * z.y.atan2(z.x)).sin() + 0.5;
		self.sum += self.last;
	}
}

/// The external angle a point's orbit escapes along, which turns once around the set. It sums the first value's angle and each later value's turn
/// past the raised value before it, with weights shrinking by the power.
#[derive(Clone, Copy)]
struct EscapeAngle {
	radians: f32,
	/// The weight of the next value's turn past the raised one before it, which also flips its sign with each mirroring of the Tricorn.
	weight: f32,
	/// The angle that raising the last value to the power turned it to.
	raised: f32,
}

impl Default for EscapeAngle {
	fn default() -> Self {
		Self { radians: 0., weight: 1., raised: 0. }
	}
}

impl EscapeAngle {
	/// Adds the value `z` the orbit reached on `iteration`, given the fractal and its power.
	fn visit(&mut self, iteration: u32, z: Vec2, fractal: MandelbrotFractal, power: f32) {
		let angle = z.y.atan2(z.x);
		if iteration == 1 {
			self.radians = angle;
		} else {
			self.weight /= if fractal == MandelbrotFractal::Tricorn { -power } else { power };
			let turn = angle - self.raised;
			self.radians += self.weight * (turn - TAU * (turn / TAU).round());
		}

		let folded = match fractal {
			MandelbrotFractal::ClassicMandelbrot => angle,
			MandelbrotFractal::BurningShip => z.y.abs().atan2(z.x.abs()),
			MandelbrotFractal::Tricorn => -angle,
		};
		self.raised = power * folded;
	}
}

/// What a point's orbit records about its values as it iterates, for the colorings drawn from them.
#[derive(Clone, Copy)]
struct OrbitRecord {
	/// The squared length of the first value, of the value closest to the origin, and the iteration reaching it.
	first: f32,
	closest: f32,
	closest_iteration: u32,
	/// The sum of the natural logarithm of how much the formula scales distances at each value.
	log_derivative_sum: f32,
}

impl Default for OrbitRecord {
	fn default() -> Self {
		Self {
			first: 0.,
			closest: f32::INFINITY,
			closest_iteration: 0,
			log_derivative_sum: 0.,
		}
	}
}

impl OrbitRecord {
	/// Records the value `z` the orbit reached on `iteration`, given the formula's power.
	fn visit(&mut self, iteration: u32, z: Vec2, power: f32) {
		let length_squared = z.length_squared();
		if iteration == 1 {
			self.first = length_squared;
		}
		if length_squared < self.closest {
			self.closest = length_squared;
			self.closest_iteration = iteration;
		}

		// The formula's derivative at z is the power times z raised to one less, whatever folds it, so its size has a logarithm of ln(d) + (d - 1) ln|z|
		self.log_derivative_sum += power.ln() + (power - 1.) * 0.5 * length_squared.max(f32::MIN_POSITIVE).ln();
	}

	/// How close the orbit came to the origin relative to its first value, which is full brightness for an orbit escaping before it recorded any value.
	fn closest_approach(&self) -> f32 {
		if self.closest_iteration == 0 {
			1.
		} else if self.first > 0. {
			(self.closest / self.first).sqrt()
		} else {
			0.
		}
	}

	/// The iteration that came closest to the origin, as a brightness that rises toward 1 with later iterations.
	fn closest_iteration_brightness(&self) -> f32 {
		1. - 1. / self.closest_iteration.max(1) as f32
	}
}

/// An orbit that the kernel steps through, giving its value in 32 bits.
trait Orbit {
	fn value(&self) -> Vec2;
	fn step(&mut self);
}

/// An orbit iterated directly in 64 bits.
#[derive(Clone)]
struct DirectOrbit<'a> {
	setup: &'a MandelbrotSetup,
	z: DVec2,
	c: DVec2,
}

impl Orbit for DirectOrbit<'_> {
	fn value(&self) -> Vec2 {
		self.z.as_vec2()
	}

	fn step(&mut self) {
		self.z = self.setup.formula_in_64_bits(self.z) + self.c;
	}
}

/// An orbit followed by its offset in 32 bits from a reference orbit, at `index` of the reference orbit's values.
#[derive(Clone)]
struct PerturbedOrbit<'a> {
	setup: &'a MandelbrotSetup,
	reference: &'a ReferenceOrbit,
	index: usize,
	offset: Vec2,
	/// How far the point added each iteration lies from the reference orbit's.
	added_offset: Vec2,
}

impl Orbit for PerturbedOrbit<'_> {
	fn value(&self) -> Vec2 {
		self.reference.values[self.index] + self.offset
	}

	fn step(&mut self) {
		let values = &self.reference.values;

		// Following the reference orbit from its start again wherever that shrinks the offset, or once the reference orbit escapes, keeps the offset small (Zhuoran's rebasing)
		let z = self.value();
		let shrinks = self.index > 0 && (z - values[0]).length_squared() < self.offset.length_squared();
		if shrinks || self.index + 1 == values.len() {
			self.offset = z - values[0];
			self.index = 0;
		}

		let (reference, raised_reference) = (values[self.index], self.reference.raised_values[self.index]);
		self.offset = self.setup.perturbed_step(reference, raised_reference, self.offset) + self.added_offset;
		self.index += 1;
	}
}

impl MandelbrotSetup {
	/// How far the formula's next value, before the point is added, moves from the reference value's with an offset from it, given the reference value as the formula raises it.
	/// It's worked out from the offset itself, which keeps a small offset precise rather than subtracting two nearly equal values.
	fn perturbed_step(&self, reference: Vec2, raised_reference: Vec2, offset: Vec2) -> Vec2 {
		let (reference, offset) = match self.fractal {
			MandelbrotFractal::ClassicMandelbrot => (reference, offset),
			MandelbrotFractal::BurningShip => (reference.abs(), Vec2::new(absolute_difference(reference.x, offset.x), absolute_difference(reference.y, offset.y))),
			MandelbrotFractal::Tricorn => (conjugate(reference), conjugate(offset)),
		};

		let Some(power) = self.integer_power else {
			return raised_step(self.power as f32, reference, raised_reference, offset);
		};

		// (Z + δ)^d - Z^d = δ × Σ (Z + δ)^(d - 1 - k) Z^k
		let value = reference + offset;
		let mut sum = Vec2::ZERO;
		let mut reference_power = Vec2::X;
		for _ in 0..power {
			sum = complex_multiply(sum, value) + reference_power;
			reference_power = complex_multiply(reference_power, reference);
		}
		complex_multiply(offset, sum)
	}
}

/// How far Z^d moves, for a power `d` other than an integer, when Z moves by δ, given Z^d. It's Z^d × (exp(d × ln(1 + δ/Z)) - 1), keeping to the principal branch that raising
/// Z + δ directly takes, so the logarithm turns back a whole turn where δ carries Z across the negative real axis. The offset is then as large as Z,
/// so points carried across that seam from their reference orbit are only as precise as 32-bit floats from there on.
fn raised_step(power: f32, reference: Vec2, raised_reference: Vec2, offset: Vec2) -> Vec2 {
	// Far offsets, and zero reference values like the start of the Mandelbrot set's orbits, raise the value directly
	if offset.length_squared() >= OFFSET_STEP_REACH * OFFSET_STEP_REACH * reference.length_squared() {
		return complex_power(reference + offset, power) - raised_reference;
	}

	let mut logarithm = complex_logarithm_of_one_plus(complex_divide(offset, reference));
	let crosses = reference.x < 0. && (reference.y >= 0.) != (reference.y + offset.y >= 0.);
	if crosses {
		logarithm.y += if reference.y >= 0. { -TAU } else { TAU };
	}
	complex_multiply(raised_reference, complex_exponential_less_one(logarithm * power))
}

/// ln(1 + u), summed as a series for small `u`.
fn complex_logarithm_of_one_plus(u: Vec2) -> Vec2 {
	if u.length_squared() >= SERIES_REACH * SERIES_REACH {
		let value = u + Vec2::X;
		return Vec2::new(0.5 * value.length_squared().ln(), value.y.atan2(value.x));
	}

	// u - u²/2 + u³/3 - ..., by Horner's method
	let mut sum = Vec2::ZERO;
	for term in (1..=LOGARITHM_TERMS).rev() {
		let sign = if term % 2 == 1 { 1. } else { -1. };
		sum = Vec2::new(sign / term as f32, 0.) + complex_multiply(u, sum);
	}
	complex_multiply(u, sum)
}

/// exp(w) - 1, summed as a series for small `w`.
fn complex_exponential_less_one(w: Vec2) -> Vec2 {
	if w.length_squared() >= SERIES_REACH * SERIES_REACH {
		let magnitude = w.x.exp();
		return Vec2::new(magnitude * w.y.cos() - 1., magnitude * w.y.sin());
	}

	// w + w²/2! + w³/3! + ..., by Horner's method
	let mut sum = Vec2::X;
	for term in (2..=EXPONENTIAL_TERMS).rev() {
		sum = Vec2::X + complex_multiply(w, sum) / term as f32;
	}
	complex_multiply(w, sum)
}

/// |x + δ| - |x|, worked out without subtracting two nearly equal values.
fn absolute_difference(x: f32, offset: f32) -> f32 {
	match (x >= 0., x + offset >= 0.) {
		(true, true) => offset,
		(true, false) => -2. * x - offset,
		(false, true) => 2. * x + offset,
		(false, false) => -offset,
	}
}

/// The period of the main cardioid or the bulb on its left if the point lies in one, worked out from its position alone.
pub fn cardioid_or_bulb_period(x: f32, y: f32) -> Option<u32> {
	let shifted = x - 0.25;
	let squared = shifted * shifted + y * y;
	if squared * (squared + shifted) <= 0.25 * y * y {
		return Some(1);
	}
	if (x + 1.) * (x + 1.) + y * y <= 1. / 16. {
		return Some(2);
	}
	None
}

/// The length of `v`, without its squared length overflowing.
fn length_without_overflow(v: Vec2) -> f32 {
	let largest = v.x.abs().max(v.y.abs());
	if largest == 0. || !largest.is_finite() {
		return largest;
	}
	largest * (v / largest).length()
}

fn conjugate(z: Vec2) -> Vec2 {
	Vec2::new(z.x, -z.y)
}

fn complex_multiply(a: Vec2, b: Vec2) -> Vec2 {
	Vec2::new(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x)
}

/// The principal square root of `z`.
fn complex_square_root(z: Vec2) -> Vec2 {
	let length = z.length();
	let real = (0.5 * (length + z.x)).max(0.).sqrt();
	let imaginary = (0.5 * (length - z.x)).max(0.).sqrt();
	Vec2::new(real, if z.y < 0. { -imaginary } else { imaginary })
}

fn complex_divide(a: Vec2, b: Vec2) -> Vec2 {
	complex_multiply(a, conjugate(b)) / b.length_squared()
}

fn complex_multiply_64(a: DVec2, b: DVec2) -> DVec2 {
	DVec2::new(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x)
}

/// `z` raised to `power` in 32 bits, by repeated multiplication for integer powers and in polar form for others.
fn complex_power(z: Vec2, power: f32) -> Vec2 {
	if power.fract() == 0. && power >= 0. {
		return (0..power as u32).fold(Vec2::X, |product, _| complex_multiply(product, z));
	}
	let magnitude = z.length_squared().powf(0.5 * power);
	let angle = z.y.atan2(z.x) * power;
	Vec2::new(angle.cos(), angle.sin()) * magnitude
}

/// `z` raised to `power` in 64 bits, by repeated multiplication for integer powers and in polar form for others.
fn complex_power_64(z: DVec2, power: f64) -> DVec2 {
	if power.fract() == 0. && power >= 0. {
		return (0..power as u32).fold(DVec2::X, |product, _| complex_multiply_64(product, z));
	}
	let magnitude = z.length_squared().powf(0.5 * power);
	// Adding zero turns -0 into 0, so values on the negative real axis take the side of the seam that the perturbed step's test gives them
	let angle = (z.y + 0.).atan2(z.x) * power;
	DVec2::new(angle.cos(), angle.sin()) * magnitude
}

#[cfg(test)]
mod tests {
	use super::*;

	fn settings() -> MandelbrotSettings {
		MandelbrotSettings {
			scale: 1.,
			fractal: MandelbrotFractal::ClassicMandelbrot,
			power: 2.,
			iterations: 256,
			mandelbrot_to_julia: 0.,
			mandelbrot_slice: DVec2::ZERO,
			julia_slice: DVec2::new(-0.8, 0.156),
			exterior: MandelbrotExterior::SmoothEscape,
			frequency: 5,
			interior: MandelbrotInterior::Flat,
		}
	}

	/// The samples of a `size` by `size` grid across `area`, following a reference orbit for it, and iterated directly in 64 bits.
	fn perturbed_and_direct(setup: &MandelbrotSetup, area: [DVec2; 2], size: usize) -> Vec<(f32, f32)> {
		samples_following(setup, &setup.reference_orbit_across(area).expect("an area has a reference orbit"), area, size)
	}

	/// The samples of a `size` by `size` grid across `area`, following `reference`, and iterated directly in 64 bits.
	fn samples_following(setup: &MandelbrotSetup, reference: &ReferenceOrbit, area: [DVec2; 2], size: usize) -> Vec<(f32, f32)> {
		(0..size * size)
			.map(|index| {
				let fraction = (DVec2::new((index % size) as f64, (index / size) as f64) + 0.5) / size as f64;
				let point = area[0] + (area[1] - area[0]) * fraction;
				(setup.sample(point, Some(reference), false), setup.sample(point, None, false))
			})
			.collect()
	}

	/// The share of the samples where following the reference orbit agrees with iterating directly, within `tolerance`.
	fn agreement(samples: &[(f32, f32)], tolerance: f32) -> f32 {
		samples.iter().filter(|(perturbed, direct)| (perturbed - direct).abs() <= tolerance).count() as f32 / samples.len() as f32
	}

	#[test]
	fn the_escape_brightens_smoothly_without_bands() {
		let setup = MandelbrotSetup::new(&settings());

		// Along a line from far outside toward the set, neighboring points differ only slightly, where whole iteration counts jumped
		let samples: Vec<f32> = (0..=1000).map(|step| setup.sample(DVec2::new(-2.5 + step as f64 * 0.0005, 0.6), None, false)).collect();
		let largest_step = samples.windows(2).map(|pair| (pair[1] - pair[0]).abs()).fold(0., f32::max);
		assert!(largest_step < 0.01, "the largest step between neighbors is {largest_step}");
		assert!(samples.last().unwrap() > samples.first().unwrap(), "the brightness should rise toward the set");
	}

	#[test]
	fn the_discrete_escape_steps_down_to_the_start_of_each_smooth_band() {
		let smooth = MandelbrotSetup::new(&settings());
		let discrete = MandelbrotSetup::new(&MandelbrotSettings {
			exterior: MandelbrotExterior::DiscreteEscape,
			..settings()
		});

		// Along a line toward the set, never more than one band's step below the smooth escape, which is ln(2) / ln(257) at most
		let mut levels = Vec::new();
		for step in 0..=1000 {
			let point = DVec2::new(-2. + step as f64 * 0.0014, 0.6);
			let (smooth, discrete) = (smooth.sample(point, None, false), discrete.sample(point, None, false));
			assert!(
				discrete <= smooth + 1e-6 && smooth - discrete < 0.125,
				"at {point} the discrete escape {discrete} isn't just below the smooth {smooth}"
			);
			levels.push(discrete.to_bits());
		}
		levels.dedup();
		assert!((3..50).contains(&levels.len()), "{} bands along the line", levels.len());
	}

	#[test]
	fn following_a_reference_orbit_agrees_with_iterating_in_64_bits() {
		for power in [2., 2.5] {
			for fractal in [MandelbrotFractal::ClassicMandelbrot, MandelbrotFractal::BurningShip, MandelbrotFractal::Tricorn] {
				for exterior in [
					MandelbrotExterior::SmoothEscape,
					MandelbrotExterior::Proximity,
					MandelbrotExterior::Angle,
					MandelbrotExterior::ClosestApproach,
					MandelbrotExterior::ClosestIteration,
					MandelbrotExterior::Tendrils,
					MandelbrotExterior::Rays,
				] {
					let setup = MandelbrotSetup::new(&MandelbrotSettings {
						power,
						fractal,
						exterior,
						..settings()
					});
					let samples = perturbed_and_direct(&setup, [DVec2::new(-0.8, 0.05), DVec2::new(-0.7, 0.15)], 24);
					let agreeing = agreement(&samples, 0.01);
					assert!(agreeing > 0.9, "{fractal:?} to the power {power} with {exterior:?} agrees on {agreeing}");
				}
			}
		}
	}

	#[test]
	fn deep_zooms_stay_precise() {
		// Ten billion times closer than the whole set, where 32-bit floats can't tell the pixels apart, in Seahorse Valley and on the edge of the set with a power of 2.5
		let views = [
			(2., DVec2::new(-0.743_643_887_037_151, 0.131_825_904_205_33)),
			(2.5, DVec2::new(0.172_533_641_457_266_55, 0.688_200_693_278_849_8)),
		];
		for (power, center) in views {
			let setup = MandelbrotSetup::new(&MandelbrotSettings {
				power,
				iterations: 3000,
				..settings()
			});
			let half = DVec2::splat(1e-10);
			let samples = perturbed_and_direct(&setup, [center - half, center + half], 24);

			let agreeing = agreement(&samples, 0.01);
			assert!(agreeing > 0.9, "following the reference orbit agrees with 64 bits on {agreeing} at a power of {power}");
			let mut distinct: Vec<u32> = samples.iter().map(|(perturbed, _)| perturbed.to_bits()).collect();
			distinct.sort_unstable();
			distinct.dedup();
			assert!(distinct.len() > 200, "only {} distinct values among the samples at a power of {power}", distinct.len());
		}
	}

	#[test]
	fn powers_other_than_integers_keep_to_the_seam_of_raising_directly() {
		// Across the negative real axis, where points take the other side of the seam from a reference orbit off the axis, or exactly on it.
		// Without keeping to the seam, about a third of the points agree.
		let area = [DVec2::new(-0.9, -0.05), DVec2::new(-0.8, 0.05)];
		for fractal in [MandelbrotFractal::ClassicMandelbrot, MandelbrotFractal::Tricorn] {
			let setup = MandelbrotSetup::new(&MandelbrotSettings { fractal, power: 2.5, ..settings() });
			let on_axis = setup.reference_orbit_across([DVec2::new(-0.85, 0.); 2]).expect("a point has a reference orbit");
			for reference in [setup.reference_orbit_across(area).expect("an area has a reference orbit"), on_axis] {
				let agreeing = agreement(&samples_following(&setup, &reference, area, 24), 0.01);
				assert!(agreeing > 0.8, "{fractal:?} following a reference orbit from {} agrees on {agreeing}", reference.point);
			}
		}
	}

	#[test]
	fn powers_other_than_integers_follow_a_reference_orbit_only_too_deep_for_32_bit_floats() {
		let (corner, shallow, deep) = (DVec2::new(-1.25, 0.), 2_f64.powi(-8), 2_f64.powi(-30));
		let reference = |power: f64, texel: f64| MandelbrotSetup::new(&MandelbrotSettings { power, ..settings() }).reference_orbit([corner, corner + texel * 256.], texel);

		assert!(reference(2., shallow).is_some());
		assert!(reference(2., deep).is_some());
		assert!(reference(2.5, shallow).is_none());
		assert!(reference(2.5, deep).is_some());
	}

	#[test]
	fn deep_zooms_of_julia_sets_stay_precise() {
		let setup = MandelbrotSetup::new(&MandelbrotSettings {
			mandelbrot_to_julia: 100.,
			..settings()
		});

		// Around a point that iterates onto the origin, where the reference orbit's start isn't the origin
		let center = DVec2::new(0.898_629, -0.086_799);
		for half in [1e-2, 1e-4, 1e-6] {
			let samples = perturbed_and_direct(&setup, [center - half, center + half], 24);
			let agreeing = agreement(&samples, 0.01);
			assert!(agreeing > 0.9, "following the reference orbit agrees with 64 bits on {agreeing} at a half width of {half}");
		}
	}

	#[test]
	fn the_sets_keep_their_symmetries() {
		let points = [DVec2::new(-0.75, 0.1), DVec2::new(0.3, 0.5), DVec2::new(-1.3, 0.05), DVec2::new(-0.1, 0.9)];

		// The Mandelbrot set mirrors across the real axis
		let mandelbrot = MandelbrotSetup::new(&settings());
		for point in points {
			let mirrored = DVec2::new(point.x, -point.y);
			assert!((mandelbrot.sample(point, None, false) - mandelbrot.sample(mirrored, None, false)).abs() < 1e-4, "{point}");
		}

		// A Julia set is symmetric through the origin
		let julia = MandelbrotSetup::new(&MandelbrotSettings {
			mandelbrot_to_julia: 100.,
			..settings()
		});
		for point in points {
			assert!((julia.sample(point, None, false) - julia.sample(-point, None, false)).abs() < 1e-4, "{point}");
		}
	}

	#[test]
	fn mandelbrot_to_julia_turns_the_slice_at_a_steady_scale() {
		// The slice turns exactly from the fractal's set to the Julia set, with a point of the plane moving the same distance through the hybrids between
		for (mandelbrot_to_julia, expected) in [(0., Some((0., 1.))), (25., None), (50., None), (75., None), (100., Some((1., 0.)))] {
			let setup = MandelbrotSetup::new(&MandelbrotSettings { mandelbrot_to_julia, ..settings() });
			let scale = (setup.start_slope * setup.start_slope + setup.added_slope * setup.added_slope).sqrt();
			assert!((scale - 1.).abs() < 1e-12, "at {mandelbrot_to_julia}% a point moves {scale} times as far");
			if let Some(expected) = expected {
				assert_eq!((setup.start_slope, setup.added_slope), expected);
			}
		}
	}

	#[test]
	fn each_slice_shapes_the_fractal_only_away_from_the_other_end() {
		let points = [DVec2::new(-0.75, 0.1), DVec2::new(0.3, 0.5), DVec2::new(-1.3, 0.05), DVec2::new(-0.1, 0.9)];
		let samples = |mandelbrot_to_julia: f64, mandelbrot_slice: DVec2, julia_slice: DVec2| {
			let setup = MandelbrotSetup::new(&MandelbrotSettings {
				mandelbrot_to_julia,
				mandelbrot_slice,
				julia_slice,
				..settings()
			});
			points.map(|point| setup.sample(point, None, false))
		};
		let (bent, other_julia) = (DVec2::new(0.3, -0.2), DVec2::new(0.285, 0.01));
		let julia_slice = settings().julia_slice;

		// The Mandelbrot Slice reshapes the set at the Mandelbrot end and does nothing at the Julia end, and the Julia Slice the reverse
		assert_ne!(samples(0., DVec2::ZERO, julia_slice), samples(0., bent, julia_slice));
		assert_eq!(samples(100., DVec2::ZERO, julia_slice), samples(100., bent, julia_slice));
		assert_eq!(samples(0., DVec2::ZERO, julia_slice), samples(0., DVec2::ZERO, other_julia));
		assert_ne!(samples(100., DVec2::ZERO, julia_slice), samples(100., DVec2::ZERO, other_julia));

		let bent_setup = MandelbrotSetup::new(&MandelbrotSettings { mandelbrot_slice: bent, ..settings() });
		assert!(!bent_setup.has_interior_test());
	}

	#[test]
	fn the_discrete_settling_darkens_toward_each_bulbs_center() {
		let setup = MandelbrotSetup::new(&MandelbrotSettings {
			interior: MandelbrotInterior::DiscreteSettling,
			..settings()
		});
		let sample = |point: DVec2| setup.sample(point, None, false);

		// The orbits from the centers of the cardioid and the bulb on its left are their cycles from the start
		assert_eq!(sample(DVec2::ZERO), 0.);
		assert_eq!(sample(DVec2::new(-1., 0.)), 0.);

		// Toward the cardioid's edge at 0.25, orbits take longer to settle
		let (inner, middle, outer) = (sample(DVec2::new(0.1, 0.)), sample(DVec2::new(0.2, 0.)), sample(DVec2::new(0.24, 0.)));
		assert!(
			0. < inner && inner < middle && middle < outer && outer < 1.,
			"the settling brightens from {inner} to {middle} to {outer}"
		);
	}

	#[test]
	fn the_interior_edge_distance_matches_the_cardioid_and_bulb() {
		let setup = MandelbrotSetup::new(&MandelbrotSettings {
			interior: MandelbrotInterior::Proximity,
			..settings()
		});

		// The multiplier is 1 - √(1 - 4c) across the cardioid, changing with the point at 2 / √(1 - 4c), and 4(c + 1) across the bulb, changing at 4
		let closed_form = |c: DVec2| {
			let (multiplier, rate) = if cardioid_or_bulb_period(c.x as f32, c.y as f32) == Some(1) {
				let root = complex_square_root((DVec2::X - c * 4.).as_vec2());
				(Vec2::X - root, 2. / root.length())
			} else {
				(((c + DVec2::X) * 4.).as_vec2(), 4.)
			};
			let distance = (1. - multiplier.length_squared()) / rate;
			-distance.log2() / DISTANCE_DOUBLINGS
		};

		for c in [DVec2::new(0.2, 0.), DVec2::new(-0.1, 0.3), DVec2::new(-1., 0.), DVec2::new(-1.1, 0.1)] {
			let (iterated, expected) = (setup.sample(c, None, false), closed_form(c));
			assert!((iterated - expected).abs() < 1e-3, "at {c}, iterating gives {iterated} but the closed form {expected}");
		}
	}

	#[test]
	fn the_external_angle_turns_once_around_the_set() {
		let setup = MandelbrotSetup::new(&MandelbrotSettings {
			exterior: MandelbrotExterior::Angle,
			..settings()
		});
		let sample = |point: DVec2| setup.sample(point, None, false);

		// Far out, it's the point's own direction, which straight up is a quarter turn past the seam on the left
		assert!((sample(DVec2::new(0., 1000.)) - 0.75).abs() < 1e-3);

		// Above the root of the bulb on the left, it's the third of a turn of the ray landing there
		assert!((sample(DVec2::new(-0.75, 0.3)) - (1. / 3. + 0.5)).abs() < 1e-3);

		// Mirrored across the real axis, it turns the other way
		for point in [DVec2::new(-0.75, 0.3), DVec2::new(0.4, 0.4), DVec2::new(-1.5, 0.2)] {
			let (above, below) = (sample(point), sample(DVec2::new(point.x, -point.y)));
			assert!((above + below - 1.).abs() < 1e-4, "{point} gives {above} and its mirror {below}");
		}

		// Along a line above the set, it changes smoothly across the bands where points escape one iteration apart
		let samples: Vec<f32> = (0..=1000).map(|step| sample(DVec2::new(-2.5 + step as f64 * 0.0035, 1.2))).collect();
		let largest_step = samples.windows(2).map(|pair| (pair[1] - pair[0]).abs()).fold(0., f32::max);
		assert!(largest_step < 0.005, "the largest step between neighbors is {largest_step}");
	}

	#[test]
	fn the_thumbnail_frames_the_fractals_body() {
		// The Mandelbrot set's body spans about -1.75 to 0.37 across and -1.1 to 1.1 down, which the thumbnail frames in the pattern's units, short of the whole scan
		let scale = 100.;
		let area_in_plane = |settings: MandelbrotSettings| MandelbrotSetup::new(&settings).thumbnail_area().map(|corner| corner / scale);
		let [min, max] = area_in_plane(MandelbrotSettings { scale, ..settings() });
		assert!(min.x < -1.75 && max.x > 0.37 && min.y < -1.1 && max.y > 1.1, "the set's area is {min} to {max}");
		assert!(min.x > -2.6 && max.x < 1. && min.y > -1.6 && max.y < 1.6, "the set's area is {min} to {max}");

		// The Julia set at the other end of Mandelbrot-to-Julia is centered on the origin
		let [min, max] = area_in_plane(MandelbrotSettings {
			scale,
			mandelbrot_to_julia: 100.,
			..settings()
		});
		assert!((min + max).length() < 0.1 * (max - min).length(), "the Julia set's area is {min} to {max}");
		assert!(max.x > 1.2 && max.x < 2.2, "the Julia set's area is {min} to {max}");
	}

	#[test]
	fn the_flat_exterior_and_interior_leave_a_silhouette_of_the_set() {
		let setup = MandelbrotSetup::new(&MandelbrotSettings {
			exterior: MandelbrotExterior::Flat,
			..settings()
		});
		assert_eq!(setup.sample(DVec2::new(1., 1.), None, false), 0.);
		assert_eq!(setup.sample(DVec2::new(0.5, 0.5), None, false), 0.);
		assert_eq!(setup.sample(DVec2::new(-0.1, 0.1), None, false), 1.);
	}

	#[test]
	fn each_bulb_takes_the_brightness_of_its_period() {
		let setup = MandelbrotSetup::new(&MandelbrotSettings {
			interior: MandelbrotInterior::Period,
			..settings()
		});

		// The centers of the main cardioid and of the bulbs with periods 2 and 3, found by iterating and, for the first two, by the test of their position
		let centers = [(DVec2::ZERO, 1.), (DVec2::new(-1., 0.), 2.), (DVec2::new(-0.122_561_166_876_654, 0.744_861_766_619_744), 3.)];
		for (center, period) in centers {
			for interior_test in [false, true] {
				let brightness = setup.sample(center, None, interior_test);
				assert!((brightness - (1. - 1. / period)).abs() < 1e-5, "period {period} gave {brightness}");
			}
		}
	}

	#[test]
	fn the_interior_test_gives_the_multipliers_iterating_finds() {
		// Inside the main cardioid and the bulb on its left, away from their centers and edges
		let points = [DVec2::new(0.2, 0.), DVec2::new(-0.1, 0.3), DVec2::new(-0.4, -0.2), DVec2::new(-1.1, 0.1), DVec2::new(-0.95, -0.15)];
		for interior in [MandelbrotInterior::Radius, MandelbrotInterior::Angle, MandelbrotInterior::Rays] {
			let setup = MandelbrotSetup::new(&MandelbrotSettings { interior, ..settings() });
			for point in points {
				let (tested, iterated) = (setup.sample(point, None, true), setup.sample(point, None, false));
				assert!((tested - iterated).abs() < 1e-3, "{interior:?} at {point} is {tested} from the test but {iterated} from iterating");
			}
		}
	}

	#[test]
	fn the_interiors_shade_by_what_their_orbits_do() {
		let sample = |interior, point: DVec2| MandelbrotSetup::new(&MandelbrotSettings { interior, ..settings() }).sample(point, None, false);

		// From 0.2, the orbit climbs along the real axis toward its fixed point 0.2764 = (1 - √0.2) / 2, so it comes closest on the first iteration
		let climbing = DVec2::new(0.2, 0.);
		assert_eq!(sample(MandelbrotInterior::ClosestApproach, climbing), 1.);
		assert_eq!(sample(MandelbrotInterior::ClosestIteration, climbing), 0.);
		assert!((sample(MandelbrotInterior::FinalAngle, climbing) - 0.5).abs() < 1e-5);

		// From -1, the orbit alternates between -1 and the origin, which it reaches on the second iteration
		let alternating = DVec2::new(-1., 0.);
		assert_eq!(sample(MandelbrotInterior::ClosestApproach, alternating), 0.);
		assert_eq!(sample(MandelbrotInterior::ClosestIteration, alternating), 0.5);
		assert!((sample(MandelbrotInterior::FinalDistance, climbing) - 0.1382).abs() < 1e-3);

		// The multiplier there is 1 - √0.2, a positive real number
		assert!((sample(MandelbrotInterior::Radius, climbing) - 0.5528).abs() < 1e-3);
		assert!((sample(MandelbrotInterior::Angle, climbing) - 0.5).abs() < 1e-3);
		assert!((sample(MandelbrotInterior::Rays, climbing) - 1.).abs() < 1e-3);

		// Orbits are drawn in hardest at a bulb's center and least near its edge, where the multiplier's size nears 1
		let center = sample(MandelbrotInterior::SmoothSettling, DVec2::ZERO);
		let near_edge = sample(MandelbrotInterior::SmoothSettling, DVec2::new(0.24, 0.));
		assert!(center < 0.01, "the cardioid's center has a brightness of {center}");
		assert!((near_edge - 0.8).abs() < 0.05, "near the cardioid's edge has a brightness of {near_edge}");
	}

	#[test]
	fn the_closest_approach_exterior_shades_how_near_escaping_orbits_dip_toward_the_origin() {
		let setup = MandelbrotSetup::new(&MandelbrotSettings {
			exterior: MandelbrotExterior::ClosestApproach,
			..settings()
		});

		// Far out, the orbit only grows, so it comes closest on its first iteration
		assert_eq!(setup.sample(DVec2::new(3., 1.), None, false), 1.);

		// Above the bulb on the left, the orbit escapes after landing at -0.25 - 0.5i on its second iteration, half as far out as -1 + 0.5i on its first
		let above_bulb = setup.sample(DVec2::new(-1., 0.5), None, false);
		assert!((above_bulb - 0.5).abs() < 1e-5, "above the bulb has a brightness of {above_bulb}");
	}

	#[test]
	fn the_closest_iteration_exterior_bands_by_when_escaping_orbits_come_closest() {
		let setup = MandelbrotSetup::new(&MandelbrotSettings {
			exterior: MandelbrotExterior::ClosestIteration,
			..settings()
		});

		// Far out, the orbit comes closest on its first iteration, and above the bulb on the left, on its second
		assert_eq!(setup.sample(DVec2::new(3., 1.), None, false), 0.);
		assert_eq!(setup.sample(DVec2::new(-1., 0.5), None, false), 0.5);
	}

	#[test]
	fn the_lowest_power_keeps_escaping_values_within_32_bits() {
		// A power of 1, the node's lowest, is taken as the lowest one iterated
		assert_eq!(MandelbrotSetup::new(&MandelbrotSettings { power: 1., ..settings() }).power, POWER_RANGE.0);

		// Far enough out to escape within the iterations despite the escape radius of 2^50, with every exterior finite and within the gradient
		for exterior in [
			MandelbrotExterior::SmoothEscape,
			MandelbrotExterior::DiscreteEscape,
			MandelbrotExterior::Proximity,
			MandelbrotExterior::Angle,
			MandelbrotExterior::ClosestApproach,
			MandelbrotExterior::ClosestIteration,
			MandelbrotExterior::Tendrils,
			MandelbrotExterior::Rays,
		] {
			let setup = MandelbrotSetup::new(&MandelbrotSettings {
				power: POWER_RANGE.0,
				exterior,
				..settings()
			});
			for point in [DVec2::new(1e13, 0.), DVec2::new(-3e14, 2e14)] {
				let brightness = setup.sample(point, None, false);
				assert!((0. ..=1.).contains(&brightness), "{exterior:?} at {point} has a brightness of {brightness}");
				if exterior == MandelbrotExterior::SmoothEscape {
					assert!(brightness < 0.5, "{point} should escape early, not count as inside");
				}
			}
		}
	}

	#[test]
	fn high_powers_keep_the_distance_estimate_within_32_bits() {
		// About 1.2 units from the set, so nearly dark, where an overflowing length would read as touching it at full brightness
		let setup = MandelbrotSetup::new(&MandelbrotSettings {
			power: 12.,
			exterior: MandelbrotExterior::Proximity,
			..settings()
		});
		let brightness = setup.sample(DVec2::new(-1.5, 0.01), None, false);
		assert!(brightness < 0.1, "a point far from the set has a brightness of {brightness}");
	}

	#[test]
	fn the_interior_test_finds_the_cardioid_and_its_left_bulb() {
		assert_eq!(cardioid_or_bulb_period(0., 0.), Some(1));
		assert_eq!(cardioid_or_bulb_period(0.24, 0.), Some(1));
		assert_eq!(cardioid_or_bulb_period(-1., 0.2), Some(2));
		assert_eq!(cardioid_or_bulb_period(0.26, 0.), None);
		assert_eq!(cardioid_or_bulb_period(-1.3, 0.), None);
		assert_eq!(cardioid_or_bulb_period(-0.122, 0.745), None);
	}
}
