// The Mandelbrot settings' choices, matching the Rust side's
const FRACTAL_BURNING_SHIP: u32 = 1u;
const FRACTAL_TRICORN: u32 = 2u;
const EXTERIOR_SMOOTH_ESCAPE: u32 = 0u;
const EXTERIOR_DISCRETE_ESCAPE: u32 = 4u;
const EXTERIOR_PROXIMITY: u32 = 1u;
const EXTERIOR_TENDRILS: u32 = 2u;
const EXTERIOR_CLOSEST_APPROACH: u32 = 5u;
const EXTERIOR_FLAT: u32 = 6u;
const EXTERIOR_ANGLE: u32 = 7u;
const EXTERIOR_CLOSEST_ITERATION: u32 = 8u;
const INTERIOR_FLAT: u32 = 0u;
const INTERIOR_FINAL_DISTANCE: u32 = 1u;
const INTERIOR_PERIOD: u32 = 2u;
const INTERIOR_CLOSEST_APPROACH: u32 = 3u;
const INTERIOR_CLOSEST_ITERATION: u32 = 4u;
const INTERIOR_FINAL_ANGLE: u32 = 5u;
const INTERIOR_RADIUS: u32 = 6u;
const INTERIOR_ANGLE: u32 = 7u;
const INTERIOR_RAYS: u32 = 9u;
const INTERIOR_SMOOTH_SETTLING: u32 = 8u;
const INTERIOR_DISCRETE_SETTLING: u32 = 10u;
const INTERIOR_PROXIMITY: u32 = 11u;

// The reference orbit's values in each row of its texture
const REFERENCE_ROW_LENGTH: u32 = 1024u;

// How close a settled orbit's value must come back to where it was to count as one turn of its cycle, however close its closest return
const PERIOD_TOLERANCE: f32 = 1e-5;

// How close a settled orbit's value must come back to where it was at least once for it to count as cycling at all
const SETTLED_TOLERANCE: f32 = 1e-2;

// How close a value must come to where it lands a turn of its cycle later for the orbit to count as settled into the cycle, which sets the Discrete Settling's bands
const SETTLING_TOLERANCE: f32 = 1e-3;

// How many doublings of a point's distance to the set span the whole gradient
const DISTANCE_DOUBLINGS: f32 = 16.;

// How far an offset can reach from the reference value, relative to it, for a power other than an integer to step the offset by itself rather than raise its value directly
const OFFSET_STEP_REACH: f32 = 0.5;

// How large a number can be for the logarithm of one plus it, and its exponential less one, to be summed as series, where 32-bit functions would lose its precision
const SERIES_REACH: f32 = 0.125;

// The terms of those series, which keep them within 32-bit rounding up to their reach
const LOGARITHM_TERMS: u32 = 7u;
const EXPONENTIAL_TERMS: u32 = 5u;

const TAU: f32 = 6.283185307179586;

// The settings of a Mandelbrot-style fractal, as the CPU's kernel iterates with them
struct Mandelbrot {
	// The tile's top left corner relative to the reference point, for points following its orbit
	delta_corner: vec2<f32>,
	// The starting value, and the point added each iteration, at the plane's origin and how much each moves with a point of the plane
	start_constant: vec2<f32>,
	added_constant: vec2<f32>,
	start_slope: f32,
	added_slope: f32,
	fractal: u32,
	exterior: u32,
	interior: u32,
	iterations: u32,
	power: f32,
	// The power as an integer, or 0 if it isn't one
	integer_power: u32,
	frequency: f32,
	bailout_squared: f32,
	log_bailout: f32,
	// The reference orbit's length, or 0 if points are iterated directly
	reference_length: u32,
	// Whether points tested to lie inside the main cardioid or the bulb on its left skip their iterations
	interior_test: u32,
	padding: u32,
}

struct Parameters {
	corner: vec2<f32>,
	texel: vec2<f32>,
	mandelbrot: Mandelbrot,
}

@group(0) @binding(0) var<uniform> parameters: Parameters;
// The values of the reference orbit, in rows of `REFERENCE_ROW_LENGTH`, with each value as the formula raises it after it
@group(0) @binding(1) var reference_orbit: texture_2d<f32>;

// The brightness is already gamma-encoded, so a Gradient Map takes it as the position along its gradient.
// Under ANGLE's D3D11 backend, FXC fails to compile the reference orbit's texture reads if `mandelbrot` is called from within a `switch`.
@fragment
fn generate(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let encoded = clamp(mandelbrot(position.xy), 0., 1.);
	return vec4<f32>(vec3<f32>(encoded), 1.);
}

// A point's orbit, followed by its offset from the reference orbit if there is one, and otherwise iterated directly
struct Orbit {
	z: vec2<f32>,
	c: vec2<f32>,
	offset: vec2<f32>,
	// How far the point added each iteration lies from the reference orbit's
	added_offset: vec2<f32>,
	index: u32,
	// The reference orbit's value at `index` and as the formula raises it, and its starting value and raising
	reference: vec4<f32>,
	reference_start: vec4<f32>,
}

// What a point's orbit records about its values as it iterates, for the colorings drawn from them
struct OrbitRecord {
	// The squared length of the first value, of the value closest to the origin, and the iteration reaching it
	first: f32,
	closest: f32,
	closest_iteration: u32,
	// The sum of the natural logarithm of how much the formula scales distances at each value
	log_derivative_sum: f32,
}

// The brightness of the Mandelbrot-style fractal at the pixel `position` of the tile
fn mandelbrot(position: vec2<f32>) -> f32 {
	let settings = parameters.mandelbrot;
	let interior = settings.interior;

	// The interiors that don't need a point's iterations take a point the test finds inside the main cardioid or the bulb on its left from its position
	let known_by_test = interior == INTERIOR_FLAT || shows_cycle(interior);
	if settings.interior_test != 0u && known_by_test {
		let point = parameters.corner + position * parameters.texel;
		let period = cardioid_or_bulb_period(point);
		if period != 0u {
			// The cycle's multiplier is 1 - √(1 - 4c) across the cardioid, and 4(c + 1) across the bulb
			var multiplier = (point + vec2<f32>(1., 0.)) * 4.;
			if period == 1u {
				multiplier = vec2<f32>(1., 0.) - complex_square_root(vec2<f32>(1., 0.) - point * 4.);
			}
			return cycle_brightness(true, period, multiplier);
		}
	}

	var orbit = start_orbit(position);

	// The orbit's derivative with respect to the point, which starts at how much the starting value moves with it
	var derivative = vec2<f32>(settings.start_slope, 0.);
	var stripe_sum = 0.;
	var last_stripe = 0.;
	var escape_angle = EscapeAngle(0., 1., 0.);
	let records = settings.exterior == EXTERIOR_CLOSEST_APPROACH || settings.exterior == EXTERIOR_CLOSEST_ITERATION || interior == INTERIOR_CLOSEST_APPROACH || interior == INTERIOR_CLOSEST_ITERATION || interior == INTERIOR_SMOOTH_SETTLING;
	var record = OrbitRecord(0., 3.4e38, 0u, 0.);

	for (var iteration = 1u; iteration <= settings.iterations; iteration++) {
		if settings.exterior == EXTERIOR_PROXIMITY {
			derivative = derivative_step(orbit_value(orbit), derivative);
		}

		orbit_step(&orbit);
		let z = orbit_value(orbit);

		if settings.exterior == EXTERIOR_TENDRILS {
			last_stripe = 0.5 * sin(settings.frequency * atan2(z.y, z.x)) + 0.5;
			stripe_sum += last_stripe;
		}

		if settings.exterior == EXTERIOR_ANGLE {
			visit_escape_angle(&escape_angle, iteration, z);
		}

		if dot(z, z) > settings.bailout_squared {
			return exterior(iteration, z, derivative, stripe_sum, last_stripe, escape_angle, record);
		}

		if records {
			record_value(&record, iteration, z);
		}
	}

	return interior_brightness(&orbit, record, position);
}

// The orbit of the point at the pixel `position` of the tile, before its first iteration
fn start_orbit(position: vec2<f32>) -> Orbit {
	let settings = parameters.mandelbrot;
	var orbit: Orbit;
	if follows_reference() {
		let offset = settings.delta_corner + position * parameters.texel;
		orbit.offset = offset * settings.start_slope;
		orbit.added_offset = offset * settings.added_slope;
		orbit.reference_start = reference_value(0u);
		orbit.reference = orbit.reference_start;
	} else {
		let point = parameters.corner + position * parameters.texel;
		orbit.z = point * settings.start_slope + settings.start_constant;
		orbit.c = point * settings.added_slope + settings.added_constant;
	}
	return orbit;
}

// The external angle a point's orbit escapes along, which turns once around the set. It sums the first value's angle and each later value's turn
// past the raised value before it, with weights shrinking by the power.
struct EscapeAngle {
	radians: f32,
	// The weight of the next value's turn past the raised one before it, which also flips its sign with each mirroring of the Tricorn
	weight: f32,
	// The angle that raising the last value to the power turned it to
	raised: f32,
}

// Adds the value `z` the orbit reached on `iteration` to its external angle
fn visit_escape_angle(escape_angle: ptr<function, EscapeAngle>, iteration: u32, z: vec2<f32>) {
	let settings = parameters.mandelbrot;
	let angle = atan2(z.y, z.x);
	if iteration == 1u {
		(*escape_angle).radians = angle;
	} else {
		(*escape_angle).weight /= select(settings.power, -settings.power, settings.fractal == FRACTAL_TRICORN);
		let turn = angle - (*escape_angle).raised;
		(*escape_angle).radians += (*escape_angle).weight * (turn - TAU * round(turn / TAU));
	}

	var folded = angle;
	if settings.fractal == FRACTAL_BURNING_SHIP {
		folded = atan2(abs(z.y), abs(z.x));
	} else if settings.fractal == FRACTAL_TRICORN {
		folded = -angle;
	}
	(*escape_angle).raised = settings.power * folded;
}

// Records the value `z` the orbit reached on `iteration`
fn record_value(record: ptr<function, OrbitRecord>, iteration: u32, z: vec2<f32>) {
	let power = parameters.mandelbrot.power;
	let length_squared = dot(z, z);
	if iteration == 1u {
		(*record).first = length_squared;
	}
	if length_squared < (*record).closest {
		(*record).closest = length_squared;
		(*record).closest_iteration = iteration;
	}

	// The formula's derivative at z is the power times z raised to one less, whatever folds it, so its size has a logarithm of ln(d) + (d - 1) ln|z|
	(*record).log_derivative_sum += log(power) + (power - 1.) * 0.5 * log(max(length_squared, 1.175494e-38));
}

// How close the orbit came to the origin relative to its first value, which is full brightness for an orbit escaping before it recorded any value
fn closest_approach(record: OrbitRecord) -> f32 {
	if record.closest_iteration == 0u {
		return 1.;
	}
	return select(0., sqrt(record.closest / record.first), record.first > 0.);
}

// The iteration that came closest to the origin, as a brightness that rises toward 1 with later iterations
fn closest_iteration_brightness(record: OrbitRecord) -> f32 {
	return 1. - 1. / f32(max(record.closest_iteration, 1u));
}

fn follows_reference() -> bool {
	return parameters.mandelbrot.reference_length != 0u;
}

fn orbit_value(orbit: Orbit) -> vec2<f32> {
	if follows_reference() {
		return orbit.reference.xy + orbit.offset;
	}
	return orbit.z;
}

fn orbit_step(orbit: ptr<function, Orbit>) {
	let settings = parameters.mandelbrot;
	if !follows_reference() {
		(*orbit).z = complex_power(formula_input((*orbit).z), settings.power) + (*orbit).c;
		return;
	}

	// Following the reference orbit from its start again wherever that shrinks the offset, or once the reference orbit escapes, keeps the offset small (Zhuoran's rebasing)
	let z = (*orbit).reference.xy + (*orbit).offset;
	let rebased = z - (*orbit).reference_start.xy;
	let shrinks = (*orbit).index > 0u && dot(rebased, rebased) < dot((*orbit).offset, (*orbit).offset);
	if shrinks || (*orbit).index + 1u == settings.reference_length {
		(*orbit).offset = rebased;
		(*orbit).index = 0u;
		(*orbit).reference = (*orbit).reference_start;
	}

	(*orbit).offset = perturbed_step((*orbit).reference.xy, (*orbit).reference.zw, (*orbit).offset) + (*orbit).added_offset;
	(*orbit).index += 1u;
	(*orbit).reference = reference_value((*orbit).index);
}

fn reference_value(index: u32) -> vec4<f32> {
	return textureLoad(reference_orbit, vec2<i32>(i32(index % REFERENCE_ROW_LENGTH), i32(index / REFERENCE_ROW_LENGTH)), 0);
}

// What the formula does to the value before raising it to the power
fn formula_input(z: vec2<f32>) -> vec2<f32> {
	let fractal = parameters.mandelbrot.fractal;
	if fractal == FRACTAL_BURNING_SHIP {
		return abs(z);
	}
	if fractal == FRACTAL_TRICORN {
		return vec2<f32>(z.x, -z.y);
	}
	return z;
}

// How far the formula's next value, before the point is added, moves from the reference value's with an offset from it, given the reference value as the formula raises it.
// It's worked out from the offset itself, which keeps a small offset precise rather than subtracting two nearly equal values.
fn perturbed_step(orbit_reference: vec2<f32>, raised_reference: vec2<f32>, orbit_offset: vec2<f32>) -> vec2<f32> {
	let settings = parameters.mandelbrot;
	var reference = orbit_reference;
	var offset = orbit_offset;
	if settings.fractal == FRACTAL_BURNING_SHIP {
		offset = vec2<f32>(absolute_difference(reference.x, offset.x), absolute_difference(reference.y, offset.y));
		reference = abs(reference);
	}
	if settings.fractal == FRACTAL_TRICORN {
		offset = vec2<f32>(offset.x, -offset.y);
		reference = vec2<f32>(reference.x, -reference.y);
	}

	if settings.integer_power == 0u {
		return raised_step(reference, raised_reference, offset);
	}

	// (Z + δ)^d - Z^d = δ × Σ (Z + δ)^(d - 1 - k) Z^k
	let value = reference + offset;
	var sum = vec2<f32>(0.);
	var reference_power = vec2<f32>(1., 0.);
	for (var k = 0u; k < settings.integer_power; k++) {
		sum = complex_multiply(sum, value) + reference_power;
		reference_power = complex_multiply(reference_power, reference);
	}
	return complex_multiply(offset, sum);
}

// How far Z^d moves, for a power `d` other than an integer, when Z moves by δ, given Z^d. It's Z^d × (exp(d × ln(1 + δ/Z)) - 1), keeping to the principal branch that raising
// Z + δ directly takes, so the logarithm turns back a whole turn where δ carries Z across the negative real axis.
fn raised_step(reference: vec2<f32>, raised_reference: vec2<f32>, offset: vec2<f32>) -> vec2<f32> {
	let power = parameters.mandelbrot.power;

	// Far offsets, and zero reference values like the start of the Mandelbrot set's orbits, raise the value directly
	if dot(offset, offset) >= OFFSET_STEP_REACH * OFFSET_STEP_REACH * dot(reference, reference) {
		return complex_power(reference + offset, power) - raised_reference;
	}

	var logarithm = complex_logarithm_of_one_plus(complex_divide(offset, reference));
	let crosses = reference.x < 0. && (reference.y >= 0.) != (reference.y + offset.y >= 0.);
	if crosses {
		logarithm.y += select(TAU, -TAU, reference.y >= 0.);
	}
	return complex_multiply(raised_reference, complex_exponential_less_one(logarithm * power));
}

// ln(1 + u), summed as a series for small `u`
fn complex_logarithm_of_one_plus(u: vec2<f32>) -> vec2<f32> {
	if dot(u, u) >= SERIES_REACH * SERIES_REACH {
		let value = u + vec2<f32>(1., 0.);
		return vec2<f32>(0.5 * log(dot(value, value)), atan2(value.y, value.x));
	}

	// u - u²/2 + u³/3 - ..., by Horner's method
	var sum = vec2<f32>(0.);
	for (var term = LOGARITHM_TERMS; term >= 1u; term--) {
		let sign = select(-1., 1., term % 2u == 1u);
		sum = vec2<f32>(sign / f32(term), 0.) + complex_multiply(u, sum);
	}
	return complex_multiply(u, sum);
}

// exp(w) - 1, summed as a series for small `w`
fn complex_exponential_less_one(w: vec2<f32>) -> vec2<f32> {
	if dot(w, w) >= SERIES_REACH * SERIES_REACH {
		let magnitude = exp(w.x);
		return vec2<f32>(magnitude * cos(w.y) - 1., magnitude * sin(w.y));
	}

	// w + w²/2! + w³/3! + ..., by Horner's method
	var sum = vec2<f32>(1., 0.);
	for (var term = EXPONENTIAL_TERMS; term >= 2u; term--) {
		sum = vec2<f32>(1., 0.) + complex_multiply(w, sum) / f32(term);
	}
	return complex_multiply(w, sum);
}

// |x + δ| - |x|, worked out without subtracting two nearly equal values
fn absolute_difference(x: f32, offset: f32) -> f32 {
	if x >= 0. {
		return select(-2. * x - offset, offset, x + offset >= 0.);
	}
	return select(-offset, 2. * x + offset, x + offset >= 0.);
}

// The derivative of the orbit's next value with respect to the point, given its current value `z` and derivative
fn derivative_step(z: vec2<f32>, derivative: vec2<f32>) -> vec2<f32> {
	return chain_step(z, derivative) + vec2<f32>(parameters.mandelbrot.added_slope, 0.);
}

// The formula's derivative at the value `z` times `derivative`, chaining the derivative of the value before it through one iteration
fn chain_step(orbit_z: vec2<f32>, orbit_derivative: vec2<f32>) -> vec2<f32> {
	let settings = parameters.mandelbrot;
	var z = orbit_z;
	var derivative = orbit_derivative;
	switch settings.fractal {
		case FRACTAL_BURNING_SHIP: {
			derivative *= vec2<f32>(select(-1., 1., z.x >= 0.), select(-1., 1., z.y >= 0.));
			z = abs(z);
		}
		case FRACTAL_TRICORN: {
			z = vec2<f32>(z.x, -z.y);
			derivative = vec2<f32>(derivative.x, -derivative.y);
		}
		default: {}
	}
	return complex_multiply(complex_power(z, settings.power - 1.) * settings.power, derivative);
}

// The brightness of a point that escaped on `iteration` with the value `z`
fn exterior(iteration: u32, z: vec2<f32>, derivative: vec2<f32>, stripe_sum: f32, last_stripe: f32, escape_angle: EscapeAngle, record: OrbitRecord) -> f32 {
	let settings = parameters.mandelbrot;

	// How far past the bailout radius the value landed, from 0 just past it to 1 at the bailout radius raised to the power
	let log_magnitude = 0.5 * log(dot(z, z));
	let overshoot = clamp(log(log_magnitude / settings.log_bailout) / log(settings.power), 0., 1.);
	let escape = max(f32(iteration) - overshoot, 0.);

	var value: f32;
	switch settings.exterior {
		case EXTERIOR_FLAT: {
			value = 0.;
		}
		case EXTERIOR_ANGLE: {
			value = fract(escape_angle.radians / TAU + 0.5);
		}
		case EXTERIOR_SMOOTH_ESCAPE: {
			value = log(escape + 1.) / log(f32(settings.iterations) + 1.);
		}
		// The smooth escape at the start of its band, where the value lands farthest past the bailout radius
		case EXTERIOR_DISCRETE_ESCAPE: {
			value = log(f32(iteration)) / log(f32(settings.iterations) + 1.);
		}
		case EXTERIOR_PROXIMITY: {
			let distance = 2. * exp(log_magnitude) * log_magnitude / length_without_overflow(derivative);
			value = select(1., max(-log2(distance) / DISTANCE_DOUBLINGS, 0.), distance > 0. && distance < 3.4e38);
		}
		case EXTERIOR_CLOSEST_APPROACH: {
			value = closest_approach(record);
		}
		case EXTERIOR_CLOSEST_ITERATION: {
			value = closest_iteration_brightness(record);
		}
		// Blended between the averages with and without the last value, so the stripes run on unbroken where points escape one iteration later
		case EXTERIOR_TENDRILS: {
			let average = stripe_sum / f32(iteration);
			let previous = select(average, (stripe_sum - last_stripe) / f32(iteration - 1u), iteration > 1u);
			value = previous + (average - previous) * (1. - overshoot);
		}
		// The angle scales by the power across the overshoot, so the lines run on unbroken where points escape one iteration later.
		// The Burning Ship folds the value into a quarter turn before raising it to the power, so its lines take that angle at twice the frequency to join up the same way.
		default: {
			var angle = atan2(z.y, z.x);
			var frequency = settings.frequency;
			if settings.fractal == FRACTAL_BURNING_SHIP {
				angle = atan2(abs(z.y), abs(z.x));
				frequency = 2. * settings.frequency;
			}
			value = 0.5 * cos(frequency * pow(settings.power, 1. - overshoot) * angle) + 0.5;
		}
	}

	return value;
}

// The brightness of a point inside the set at the pixel `position`, given its orbit after all its iterations and what it recorded along the way
fn interior_brightness(orbit: ptr<function, Orbit>, record: OrbitRecord, position: vec2<f32>) -> f32 {
	let settings = parameters.mandelbrot;
	let interior = settings.interior;
	if interior == INTERIOR_FLAT {
		return 1.;
	}
	// Finding the cycle once for every interior drawn from it, since the shader compiler copies each call's loops in again
	if shows_cycle(interior) || interior == INTERIOR_DISCRETE_SETTLING || interior == INTERIOR_PROXIMITY {
		let cycle = settled_cycle(orbit);
		if !cycle.found {
			return 1.;
		}
		if interior == INTERIOR_DISCRETE_SETTLING {
			return discrete_settling(cycle.period, position);
		}
		if interior == INTERIOR_PROXIMITY {
			return select(0., clamp(-log2(cycle.edge_distance) / DISTANCE_DOUBLINGS, 0., 1.), cycle.edge_distance < 3.4e38);
		}
		return cycle_brightness(cycle.found, cycle.period, cycle.multiplier);
	}

	let z = orbit_value(*orbit);
	var value: f32;
	if interior == INTERIOR_FINAL_DISTANCE {
		value = length(z) / 2.;
	} else if interior == INTERIOR_CLOSEST_APPROACH {
		value = closest_approach(record);
	} else if interior == INTERIOR_CLOSEST_ITERATION {
		value = closest_iteration_brightness(record);
	} else if interior == INTERIOR_FINAL_ANGLE {
		value = atan2(z.y, z.x) / TAU + 0.5;
	} else {
		// The geometric mean of how much each iteration scales distances, which is the exponential of the Lyapunov exponent
		value = exp(record.log_derivative_sum / f32(settings.iterations));
	}
	return min(value, 1.);
}

// The cycle an orbit has settled into after all its iterations, if it's `found`: its length, its multiplier, and for the Proximity interior, the distance to its bulb's edge
struct Cycle {
	found: bool,
	period: u32,
	multiplier: vec2<f32>,
	edge_distance: f32,
}

// The cycle the orbit has settled into after all its iterations, which leaves the orbit a turn of the cycle on.
// The length is the first number of iterations bringing the value back about as close as any does, and the multiplier is the derivative of that many iterations.
fn settled_cycle(orbit: ptr<function, Orbit>) -> Cycle {
	let settings = parameters.mandelbrot;

	let settled = orbit_value(*orbit);
	var found = false;
	var period = 0u;
	var multiplier = vec2<f32>(1., 0.);

	// For the Proximity interior, the derivatives around the cycle of its iterations by the value and by the point added each iteration,
	// and the second derivatives by the value twice and by both
	let edge = settings.interior == INTERIOR_PROXIMITY;
	var by_value = vec2<f32>(1., 0.);
	var by_added = vec2<f32>(0.);
	var by_value_twice = vec2<f32>(0.);
	var by_both = vec2<f32>(0.);

	// First a copy of the orbit steps through all its iterations to find its closest return, which finds the cycles of orbits still settling near a bulb's edge,
	// as no fixed tolerance would. Then the orbit steps until it comes back about that close. Both step through one call, since the shader compiler
	// copies a call's whole step in again, which slowed its first compile.
	var returning = *orbit;
	var closest = 3.4e38;
	for (var step = 0u; step < 2u * settings.iterations; step++) {
		let finding_period = step >= settings.iterations;
		if step == settings.iterations && closest >= SETTLED_TOLERANCE * SETTLED_TOLERANCE {
			break;
		}

		if finding_period && edge {
			let z = formula_input(orbit_value(*orbit));
			let first = complex_power(z, settings.power - 1.) * settings.power;
			let second = complex_power(z, settings.power - 2.) * (settings.power * (settings.power - 1.));
			by_both = complex_multiply(second, complex_multiply(by_value, by_added)) + complex_multiply(first, by_both);
			by_value_twice = complex_multiply(second, complex_multiply(by_value, by_value)) + complex_multiply(first, by_value_twice);
			by_added = complex_multiply(first, by_added) + vec2<f32>(1., 0.);
			by_value = complex_multiply(first, by_value);
		}
		if finding_period {
			multiplier = chain_step(orbit_value(*orbit), multiplier);
		}

		var stepping = returning;
		if finding_period {
			stepping = *orbit;
		}
		orbit_step(&stepping);
		let difference = orbit_value(stepping) - settled;

		if !finding_period {
			returning = stepping;
			closest = min(closest, dot(difference, difference));
		} else {
			*orbit = stepping;
			if dot(difference, difference) <= 4. * closest + PERIOD_TOLERANCE * PERIOD_TOLERANCE {
				found = true;
				period = step - settings.iterations + 1u;
				break;
			}
		}
	}

	// The multiplier changes with the point by the mixed derivative plus the second derivative times how far the cycle shifts,
	// and the edge lies about as far away as the multiplier's size has left to reach 1, divided by that rate (the interior distance estimate)
	var edge_distance = 0.;
	if edge {
		let shift = complex_divide(by_added, vec2<f32>(1., 0.) - by_value);
		let rate = length_without_overflow(by_both + complex_multiply(by_value_twice, shift)) * settings.added_slope;
		edge_distance = max((1. - dot(by_value, by_value)) / rate, 0.);
	}
	return Cycle(found, period, multiplier, edge_distance);
}

// The first iteration from which the orbit of the point at the pixel `position` repeats itself each turn of its `period`-long cycle to within the settling tolerance,
// brightest for the orbits that settle last, at the start of the band they settle in, like the discrete escape outside
fn discrete_settling(period: u32, position: vec2<f32>) -> f32 {
	let settings = parameters.mandelbrot;

	// The orbit again from its start, against itself a turn of the cycle ahead. Once the leading orbit is a turn ahead, they take turns stepping
	// through one call, since the shader compiler copies a call's whole step in again, which slowed its first compile.
	var following = start_orbit(position);
	var leading = following;
	var settling = settings.iterations;
	for (var step = 0u; step < period + 2u * settings.iterations; step++) {
		let following_turn = step >= period && (step - period) % 2u == 0u;
		if following_turn {
			let difference = orbit_value(leading) - orbit_value(following);
			if dot(difference, difference) < SETTLING_TOLERANCE * SETTLING_TOLERANCE {
				settling = (step - period) / 2u;
				break;
			}
		}

		var stepping = leading;
		if following_turn {
			stepping = following;
		}
		orbit_step(&stepping);
		if following_turn {
			following = stepping;
		} else {
			leading = stepping;
		}
	}
	return min(log(f32(settling) + 1.) / log(f32(settings.iterations) + 1.), 1.);
}

// The brightness inside the set from the length and multiplier of the cycle a point's orbit settles into, if one was `found`
fn cycle_brightness(found: bool, period: u32, multiplier: vec2<f32>) -> f32 {
	let interior = parameters.mandelbrot.interior;
	if interior == INTERIOR_FLAT {
		return 1.;
	}

	var value = 1.;
	if found && interior == INTERIOR_RADIUS {
		value = min(length(multiplier), 1.);
	} else if found && interior == INTERIOR_ANGLE {
		value = atan2(multiplier.y, multiplier.x) / TAU + 0.5;
	} else if found && interior == INTERIOR_RAYS {
		value = 0.5 * cos(parameters.mandelbrot.frequency * atan2(multiplier.y, multiplier.x)) + 0.5;
	} else if found {
		value = 1. - 1. / f32(period);
	}
	return value;
}

// Whether the interior shows the cycle a point's orbit settles into
fn shows_cycle(interior: u32) -> bool {
	return interior == INTERIOR_PERIOD || interior == INTERIOR_RADIUS || interior == INTERIOR_ANGLE || interior == INTERIOR_RAYS;
}

// The period of the main cardioid or the bulb on its left if the point lies in one, or 0, worked out from its position alone
fn cardioid_or_bulb_period(point: vec2<f32>) -> u32 {
	let shifted = point.x - 0.25;
	let squared = shifted * shifted + point.y * point.y;
	if squared * (squared + shifted) <= 0.25 * point.y * point.y {
		return 1u;
	}
	if (point.x + 1.) * (point.x + 1.) + point.y * point.y <= 1. / 16. {
		return 2u;
	}
	return 0u;
}

// The length of `v`, without its squared length overflowing
fn length_without_overflow(v: vec2<f32>) -> f32 {
	let largest = max(abs(v.x), abs(v.y));
	if largest == 0. || largest > 3.4e38 {
		return largest;
	}
	return largest * length(v / largest);
}

fn complex_multiply(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
	return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

// The principal square root of `z`
fn complex_square_root(z: vec2<f32>) -> vec2<f32> {
	let magnitude = length(z);
	let imaginary = sqrt(max(0.5 * (magnitude - z.x), 0.));
	return vec2<f32>(sqrt(max(0.5 * (magnitude + z.x), 0.)), select(imaginary, -imaginary, z.y < 0.));
}

fn complex_divide(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
	return complex_multiply(a, vec2<f32>(b.x, -b.y)) / dot(b, b);
}

// `z` raised to `power`, by repeated multiplication for integer powers and in polar form for others
fn complex_power(z: vec2<f32>, power: f32) -> vec2<f32> {
	if fract(power) == 0. && power >= 0. {
		var product = vec2<f32>(1., 0.);
		for (var k = 0u; k < u32(power); k++) {
			product = complex_multiply(product, z);
		}
		return product;
	}
	let squared = dot(z, z);
	if squared == 0. {
		return vec2<f32>(0.);
	}
	let magnitude = exp2(0.5 * power * log2(squared));
	let angle = atan2(z.y, z.x) * power;
	return vec2<f32>(cos(angle), sin(angle)) * magnitude;
}
