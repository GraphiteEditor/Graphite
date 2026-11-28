// The step functions carrying out color adjustments, which the shaders generated in `vello_adjustments.rs` call on the steps that the kernels
// of `raster-kernels` encode as `AdjustmentSteps`. Each mirrors its kernel's `map_cpu` on a straight color in the encoding `vello_adjustments.rs` gives it.
// The constants naming the kernels' codes for their settings come before this, generated from the kernels.

// Premultiplied pixels with gamma-encoded color, or straight linear color where the encodings say so
@group(0) @binding(0) var source: texture_2d<f32>;
// The step count, then for each step a texel holding how many texels it spans (as two 16-bit halves) and its kind followed by its parameters, filling the rows in order
@group(0) @binding(1) var steps: texture_2d<f32>;
// A texel locating each table that steps read (as two 16-bit halves), followed by the tables in order
@group(0) @binding(2) var tables: texture_2d<f32>;

// Whether the source holds straight linear color and whether the result is written that way, rather than as premultiplied gamma-encoded color.
// WebGL2 needs uniform structs to span a multiple of 16 bytes.
struct PassEncodings {
	straight_linear_source: u32,
	straight_linear_result: u32,
	padding: vec2<u32>,
}
@group(0) @binding(3) var<uniform> encodings: PassEncodings;

// The machine epsilons of single and double precision, which the CPU's kernels hold divisors above
const F32_EPSILON: f32 = 1.1920929e-7;
const F64_EPSILON: f32 = 2.220446e-16;

// One triangle that covers the whole target
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
	let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
	return vec4<f32>(corner * 2. - 1., 0., 1.);
}

// The straight color of the source pixel under `position`, in linear light if the source holds that, or else gamma-encoded,
// with a fully transparent pixel's color left at 0, matching `straight_linear_color`
fn source_color(position: vec4<f32>) -> vec4<f32> {
	let pixel = textureLoad(source, vec2<i32>(position.xy), 0);
	if pixel.a <= 0. {
		return vec4<f32>(0.);
	}
	if encodings.straight_linear_source != 0u {
		return pixel;
	}
	return vec4<f32>(pixel.rgb / pixel.a, min(pixel.a, 1.));
}

// The pixel a pass writes for the straight `color`, in linear light if `linear` or else gamma-encoded, in the result's encoding
fn result_pixel(color: vec4<f32>, linear: bool) -> vec4<f32> {
	if encodings.straight_linear_result != 0u {
		return select(from_gamma(color), color, linear);
	}
	let gamma = select(color, to_gamma(color), linear);
	return vec4<f32>(gamma.rgb * gamma.a, gamma.a);
}

// A count stored as its low (x) and high (y) 16-bit halves
fn joined_count(halves: vec2<f32>) -> u32 {
	return u32(halves.x) + (u32(halves.y) << 16u);
}

// The texel at `index` among the steps
fn step_texel(index: u32) -> vec4<f32> {
	let width = textureDimensions(steps).x;
	return textureLoad(steps, vec2<u32>(index % width, index / width), 0);
}

// How many texels the step with its header at `header` spans, header included
fn step_length(header: u32) -> u32 {
	return joined_count(step_texel(header).xy);
}

// The texel at `index` among the tables
fn table_texel(index: u32) -> vec4<f32> {
	let width = textureDimensions(tables).x;
	return textureLoad(tables, vec2<u32>(index % width, index / width), 0);
}

// Where the table numbered `table` starts among the tables
fn table_start(table: u32) -> u32 {
	return joined_count(table_texel(table).xy);
}

// ================
// Shared functions
// ================

// Whether `value` lies at or past a double-precision `point` held as its nearest single-precision value (x) and the remainder past that (y).
// The two steps tell them apart to double precision, unless a compiler folds them into one (as Metal's fast math or FXC may).
fn at_or_past(value: f32, point: vec2<f32>) -> bool {
	return value - point.x >= point.y;
}

// Whether `value` lies at or before a double-precision `point` held as in `at_or_past`
fn at_or_before(value: f32, point: vec2<f32>) -> bool {
	return value - point.x <= point.y;
}

// How far `value` lies past a double-precision `point` held as in `at_or_past`
fn distance_past(value: f32, point: vec2<f32>) -> f32 {
	return (value - point.x) - point.y;
}

// Matches Rust's `powf`: 1 for an exponent of 0 or a base of 1, 0 for a base of 0 with a positive exponent, and a finite result for a negative base with an integer exponent
fn rust_powf(base: f32, exponent: f32) -> f32 {
	if exponent == 0. || base == 1. {
		return 1.;
	}
	if base == 0. && exponent > 0. {
		return 0.;
	}
	if base < 0. && exponent == floor(exponent) {
		let magnitude = pow(-base, exponent);
		return select(magnitude, -magnitude, abs(exponent % 2.) == 1.);
	}
	return pow(base, exponent);
}

// Matches `srgb_to_linear`, with white kept at exactly 1 whatever the GPU's `pow` and division make of it
fn srgb_to_linear(channel: f32) -> f32 {
	if channel <= 0.04045 {
		return channel / 12.92;
	}
	if channel == 1. {
		return 1.;
	}
	return pow((channel + 0.055) / 1.055, 2.4);
}

// Matches `linear_to_srgb`, which keeps white exact too
fn linear_to_srgb(channel: f32) -> f32 {
	if channel <= 0.0031308 {
		return channel * 12.92;
	}
	if channel == 1. {
		return 1.;
	}
	return 1.055 * pow(channel, 1. / 2.4) - 0.055;
}

fn srgb_to_linear_rgb(rgb: vec3<f32>) -> vec3<f32> {
	return vec3<f32>(srgb_to_linear(rgb.r), srgb_to_linear(rgb.g), srgb_to_linear(rgb.b));
}

// A color's gamma-encoded channels and its alpha, matching `Color::to_gamma_srgb_channels`
fn to_gamma(color: vec4<f32>) -> vec4<f32> {
	return vec4<f32>(linear_to_srgb(color.r), linear_to_srgb(color.g), linear_to_srgb(color.b), color.a);
}

// A color from its gamma-encoded channels and its alpha, matching `Color::from_gamma_srgb_channels`
fn from_gamma(channels: vec4<f32>) -> vec4<f32> {
	return vec4<f32>(srgb_to_linear_rgb(channels.rgb), channels.a);
}

fn minimum_channel(rgb: vec3<f32>) -> f32 {
	return min(min(rgb.r, rgb.g), rgb.b);
}

fn maximum_channel(rgb: vec3<f32>) -> f32 {
	return max(max(rgb.r, rgb.g), rgb.b);
}

// `value` wrapped into the range from 0 to `modulus`
fn wrap_positive(value: f32, modulus: f32) -> f32 {
	return value - floor(value / modulus) * modulus;
}

// The real cube root, where `f32::cbrt` is exact and `pow` falls a few bits short until a Newton step refines it
fn cube_root(value: f32) -> f32 {
	// The power of 0 is whatever the GPU makes of it
	if value == 0. {
		return 0.;
	}

	let estimate = sign(value) * pow(abs(value), 1. / 3.);
	if estimate == 0. {
		return 0.;
	}
	return estimate - (estimate * estimate * estimate - value) / (3. * estimate * estimate);
}

// How many of `count` steps' entries `stride` texels apart from `at` hold a position (paired in xy, as in `at_or_past`) at or below `value`,
// the entries sorted by position. O(log n).
fn count_at_or_below(at: u32, stride: u32, count: u32, value: f32) -> u32 {
	var low = 0u;
	var high = count;
	while low < high {
		let middle = (low + high) / 2u;
		if at_or_past(value, step_texel(at + middle * stride).xy) {
			low = middle + 1u;
		} else {
			high = middle;
		}
	}
	return low;
}

// How many of `count` tables' entries from `at` hold an x (their first value) at or below `value`, the entries sorted by x. O(log n).
fn table_count_at_or_below(at: u32, count: u32, value: f32) -> u32 {
	var low = 0u;
	var high = count;
	while low < high {
		let middle = (low + high) / 2u;
		if table_texel(at + middle).x <= value {
			low = middle + 1u;
		} else {
			high = middle;
		}
	}
	return low;
}

// The Rec. 601 luma in the 14-bit fixed point that PSD interop depends on
fn luma_rec_601_fixed_point(rgb: vec3<f32>) -> f32 {
	return (4915. * rgb.r + 9667. * rgb.g + 1802. * rgb.b) / 16384.;
}

// The Rec. 601 luma with its weights rounded to two decimal places, matching `luma_rec_601_rounded`
fn luma_rec_601_rounded(rgb: vec3<f32>) -> f32 {
	return 0.3 * rgb.r + 0.59 * rgb.g + 0.11 * rgb.b;
}

// Matches `channel_range`
fn channel_range(rgb: vec3<f32>) -> f32 {
	return maximum_channel(rgb) - minimum_channel(rgb);
}

// Matches `set_luminosity`, shifting gamma-encoded channels from `luma` to `luminosity`, then pulling them toward it just enough to bring them back into 0..1
fn set_luminosity(rgb: vec3<f32>, luma: f32, luminosity: f32) -> vec3<f32> {
	var channels = rgb + (luminosity - luma);

	let low = minimum_channel(channels);
	if low < 0. {
		channels = luminosity + (channels - luminosity) * (luminosity / (luminosity - low));
	}
	let high = maximum_channel(channels);
	if high > 1. {
		channels = luminosity + (channels - luminosity) * ((1. - luminosity) / (high - luminosity));
	}

	return clamp(channels, vec3<f32>(0.), vec3<f32>(1.));
}

// Matches `set_saturation`, scaling gamma-encoded channels about their smallest so the largest becomes `saturation`
fn set_saturation(rgb: vec3<f32>, saturation: f32) -> vec3<f32> {
	let low = minimum_channel(rgb);
	let range = channel_range(rgb);
	if range <= 0. {
		return vec3<f32>(0.);
	}

	return (rgb - low) * (saturation / range);
}

// sRGB colorants adapted to D50, matching `SRGB_TO_XYZ_D50`
fn srgb_to_xyz_d50(rgb: vec3<f32>) -> vec3<f32> {
	return vec3<f32>(
		0.43607 * rgb.r + 0.38515 * rgb.g + 0.14307 * rgb.b,
		0.22249 * rgb.r + 0.71687 * rgb.g + 0.06061 * rgb.b,
		0.01392 * rgb.r + 0.09708 * rgb.g + 0.71410 * rgb.b
	);
}

// Matches `XYZ_D50_TO_SRGB`
fn xyz_d50_to_srgb(xyz: vec3<f32>) -> vec3<f32> {
	return vec3<f32>(
		3.134096 * xyz.x - 1.6174 * xyz.y - 0.490638 * xyz.z,
		-0.978793 * xyz.x + 1.916295 * xyz.y + 0.033454 * xyz.z,
		0.071971 * xyz.x - 0.228987 * xyz.y + 1.40538 * xyz.z
	);
}

const WHITE_XYZ_D50: vec3<f32> = vec3<f32>(0.96420, 1., 0.82491);

// Hexagon hue in degrees of three channels in any encoding, 0 for gray
fn hexagon_hue_degrees(rgb: vec3<f32>) -> f32 {
	let maximum = maximum_channel(rgb);
	let chroma = maximum - minimum_channel(rgb);
	if chroma <= 0. {
		return 0.;
	}

	var sector = (rgb.r - rgb.g) / chroma + 4.;
	if rgb.r >= maximum {
		sector = wrap_positive((rgb.g - rgb.b) / chroma, 6.);
	} else if rgb.g >= maximum {
		sector = (rgb.b - rgb.r) / chroma + 2.;
	}
	return sector * 60.;
}

// ===========
// Adjustments
// ===========

// Matches `Color::lightness_oklab`
fn lightness_oklab(rgb: vec3<f32>) -> f32 {
	let long = 0.41222147 * rgb.r + 0.53633254 * rgb.g + 0.05144599 * rgb.b;
	let medium = 0.2119035 * rgb.r + 0.6806995 * rgb.g + 0.10739696 * rgb.b;
	let short = 0.08830246 * rgb.r + 0.28171884 * rgb.g + 0.6299787 * rgb.b;

	return 0.21045426 * cube_root(long) + 0.7936178 * cube_root(medium) - 0.004072047 * cube_root(short);
}

// The method (x)
fn desaturate(color: vec4<f32>, at: u32) -> vec4<f32> {
	let method = u32(step_texel(at).x);

	var luminance = 0.;
	switch method {
		case LUMINANCE_REC_709: {
			luminance = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
		}
		case LUMA_REC_709, LUMA_REC_601: {
			// Gamma-encoded formulas are decoded as if they were a gray
			let gamma = to_gamma(color);
			let weights = select(vec3<f32>(0.299, 0.587, 0.114), vec3<f32>(0.2126, 0.7152, 0.0722), method == LUMA_REC_709);
			luminance = srgb_to_linear(weights.r * gamma.r + weights.g * gamma.g + weights.b * gamma.b);
		}
		case LIGHTNESS_OKLAB: {
			// A gray's OkLab lightness is the cube root of its linear value, so cubing gives the gray of equal lightness
			let lightness = lightness_oklab(color.rgb);
			luminance = lightness * lightness * lightness;
		}
		case CHANNELS_AVERAGE: {
			luminance = (color.r + color.g + color.b) / 3.;
		}
		case CHANNELS_MINIMUM: {
			luminance = minimum_channel(color.rgb);
		}
		case CHANNELS_MAXIMUM: {
			luminance = maximum_channel(color.rgb);
		}
		case LIGHTNESS_HSL, default: {
			// The transfer curve is monotonic, so the extremes are found first and only they are encoded
			let maximum = linear_to_srgb(maximum_channel(color.rgb));
			let minimum = linear_to_srgb(minimum_channel(color.rgb));
			luminance = srgb_to_linear((maximum + minimum) / 2.);
		}
	}

	return vec4<f32>(vec3<f32>(luminance), color.a);
}

// The exponent (x), matching `Color::apply_gamma_exponent`
fn gamma_correction(color: vec4<f32>, at: u32) -> vec4<f32> {
	let inverse = 1. / max(step_texel(at).x, 0.0001);
	return vec4<f32>(rust_powf(color.r, inverse), rust_powf(color.g, inverse), rust_powf(color.b, inverse), color.a);
}

fn make_opaque(color: vec4<f32>, at: u32) -> vec4<f32> {
	return vec4<f32>(color.rgb, 1.);
}

fn invert(color: vec4<f32>, at: u32) -> vec4<f32> {
	return vec4<f32>(1. - color.rgb, color.a);
}

// Each channel inverted past middle gray, folding it back down
fn solarize(color: vec4<f32>, at: u32) -> vec4<f32> {
	return vec4<f32>(min(color.rgb, 1. - color.rgb), color.a);
}

// Matches `apply_levels` with the gamma of 1 that every caller here passes, so its power is left out
fn apply_levels(value: f32, input_shadows: f32, input_highlights: f32, output_minimum: f32, output_maximum: f32) -> f32 {
	let highlights_minus_shadows = clamp(input_highlights - input_shadows, F32_EPSILON, 1.);
	let scaled = min(max(value - input_shadows, 0.) / highlights_minus_shadows, 1.);

	return scaled * (output_maximum - output_minimum) + output_minimum;
}

// Matches `brightness_contrast_classic`
fn brightness_contrast_classic(value: f32, brightness: f32, contrast: f32, pivot: f32) -> f32 {
	// Full contrast is a hard step, sending values at the pivot or above to white (with half a 16-bit step of slack for float ties)
	if contrast >= 1. {
		return select(0., 1., value + brightness >= pivot - 1. / 65536.);
	}

	var result = 0.;
	if contrast > 0. {
		let input_shadows = pivot * contrast - brightness;
		result = apply_levels(value, input_shadows, input_shadows + 1. - contrast, 0., 1.);
	} else {
		let output_minimum = brightness - pivot * contrast;
		result = apply_levels(value, 0., 1., output_minimum, output_minimum + 1. + contrast);
	}

	return clamp(result, 0., 1.);
}

// A brightness curve's Hermite segment at its parameter t, as its value (x) and its derivative with respect to its input (y), matching `BrightnessCurve::hermite`.
// The curve holds its slope (x), knee (y), and end slope (z).
fn brightness_curve_hermite(curve: vec3<f32>, t: f32) -> vec2<f32> {
	let length = 1. - curve.y;
	let start_tangent = length * curve.x;
	let end_tangent = length * curve.z;
	let t2 = t * t;
	let t3 = t2 * t;

	let value = (2. * t3 - 3. * t2 + 1.) * 0.5 + (t3 - 2. * t2 + t) * start_tangent + (-2. * t3 + 3. * t2) + (t3 - t2) * end_tangent;
	let derivative = ((6. * t2 - 6. * t) * 0.5 + (3. * t2 - 4. * t + 1.) * start_tangent + (-6. * t2 + 6. * t) + (3. * t2 - 2. * t) * end_tangent) / length;

	return vec2<f32>(value, derivative);
}

// Matches `BrightnessCurve::forward`
fn brightness_curve_forward(curve: vec3<f32>, x: f32) -> f32 {
	if x < curve.y {
		return curve.x * x;
	}

	let t = min((x - curve.y) / (1. - curve.y), 1.);
	return min(brightness_curve_hermite(curve, t).x, 1.);
}

// Matches `BrightnessCurve::inverse`, solving the monotone Hermite segment with a bracketed Newton iteration
fn brightness_curve_inverse(curve: vec3<f32>, y: f32) -> f32 {
	if y <= 0.5 {
		return y / curve.x;
	}

	var low = 0.;
	var high = 1.;
	var t = (y - 0.5) * 2.;
	for (var iteration = 0; iteration < 8; iteration++) {
		let hermite = brightness_curve_hermite(curve, t);
		let error = hermite.x - y;
		if error > 0. {
			high = t;
		} else {
			low = t;
		}

		let newton_step = t - error / (hermite.y * (1. - curve.y));
		t = select((low + high) * 0.5, newton_step, newton_step >= low && newton_step <= high);
	}

	return curve.y + t * (1. - curve.y);
}

fn brightness_contrast_channel(value: f32, settings: vec4<f32>, first_curve: vec3<f32>, second_curve: vec3<f32>) -> f32 {
	if settings.x > 0.5 {
		return brightness_contrast_classic(value, settings.y / 255., settings.z, settings.w);
	}

	// Negative brightness runs the same curves in reverse
	var brightened = 0.;
	if settings.y >= 0. {
		brightened = brightness_curve_forward(second_curve, brightness_curve_forward(first_curve, value));
	} else {
		brightened = brightness_curve_inverse(first_curve, brightness_curve_inverse(second_curve, value));
	}

	// Contrast pushes away from (or pulls toward) the midpoint, most strongly at the quarter tones
	let contrasted = brightened + 0.76 * settings.z * (2. * brightened - 1.) * min(brightened, 1. - brightened);
	return clamp(contrasted, 0., 1.);
}

// Whether classic (x), the brightness (y), the contrast (z), and the classic pivot (w), then the first and second brightness curves
fn brightness_contrast(color: vec4<f32>, at: u32) -> vec4<f32> {
	let settings = step_texel(at);
	let first_curve = step_texel(at + 1u).xyz;
	let second_curve = step_texel(at + 2u).xyz;

	return vec4<f32>(
		brightness_contrast_channel(color.r, settings, first_curve, second_curve),
		brightness_contrast_channel(color.g, settings, first_curve, second_curve),
		brightness_contrast_channel(color.b, settings, first_curve, second_curve),
		color.a
	);
}

// A Levels curve's black and white points, exponent, and toe end, then its toe value and starting and ending slopes, matching `LevelsCurve::apply`
fn apply_levels_curve(at: u32, value: f32) -> f32 {
	let points = step_texel(at);
	let toe = step_texel(at + 1u);
	let t = clamp((value * 255. - points.x) * 255. / (points.y - points.x), 0., 255.);

	var y = 0.;
	if t < points.w {
		let u = t / points.w;
		let hermite_start = u * u * u - 2. * u * u + u;
		let hermite_end_value = 3. * u * u - 2. * u * u * u;
		let hermite_end_slope = u * u * u - u * u;
		y = hermite_start * points.w * toe.y + hermite_end_value * toe.x + hermite_end_slope * points.w * toe.z;
	} else {
		y = 255. * rust_powf(t / 255., points.z);
	}

	// Held to the range the curve lies in, which the GPU's division and `pow` would otherwise carry white a hair past
	return clamp(y / 255., 0., 1.);
}

// A Levels curve then the output minimum (x) and maximum (y) after it, matching `LevelsStage::apply`
fn apply_levels_stage(at: u32, value: f32) -> f32 {
	let output = step_texel(at + LEVELS_CURVE_TEXELS);
	let result = apply_levels_curve(at, value) * (output.y - output.x) + output.x;

	return clamp(result, min(output.x, output.y), max(output.x, output.y));
}

// Whether there are two stages (x), then the first stage and the second, matching `LevelsChain::apply`
fn apply_levels_chain(at: u32, value: f32) -> f32 {
	let first = apply_levels_stage(at + 1u, value);
	if step_texel(at).x > 0.5 {
		return apply_levels_stage(at + 1u + LEVELS_STAGE_TEXELS, first);
	}
	return first;
}

// The red, green, and blue chains, then the alpha stage
fn levels(color: vec4<f32>, at: u32) -> vec4<f32> {
	let red = apply_levels_chain(at, color.r);
	let green = apply_levels_chain(at + LEVELS_CHAIN_TEXELS, color.g);
	let blue = apply_levels_chain(at + 2u * LEVELS_CHAIN_TEXELS, color.b);
	return vec4<f32>(red, green, blue, apply_levels_stage(at + 3u * LEVELS_CHAIN_TEXELS, color.a));
}

// The spline through a transfer curve's points, which follow their count, matching `TransferCurveEvaluator::evaluate`.
// Each point holds its x (paired, as in `at_or_past`), y, and second derivative there, then the width to the next point.
fn evaluate_transfer_curve(at: u32, x: f32) -> f32 {
	let count = u32(step_texel(at).x);
	if count == 0u {
		return x;
	}
	let first = step_texel(at + 1u);
	if count == 1u || at_or_before(x, first.xy) {
		return first.z;
	}
	let last = step_texel(at + 2u * count - 1u);
	if at_or_past(x, last.xy) {
		return last.z;
	}

	// The segment from the point at `lower` to the next, its texels two apart
	let lower = clamp(count_at_or_below(at + 1u, 2u, count, x), 1u, count - 1u) - 1u;
	let a = step_texel(at + 1u + 2u * lower);
	let width = step_texel(at + 2u + 2u * lower).x;
	let b = step_texel(at + 3u + 2u * lower);

	// The cubic segment from its two end second derivatives
	let t_b = distance_past(x, a.xy) / width;
	let t_a = 1. - t_b;
	return t_a * a.z + t_b * b.z + ((t_a * t_a * t_a - t_a) * a.w + (t_b * t_b * t_b - t_b) * b.w) * width * width / 6.;
}

fn transfer_curve_length(at: u32) -> u32 {
	return 1u + 2u * u32(step_texel(at).x);
}

// The composite, red, green, blue, and alpha transfer curves
fn curves(color: vec4<f32>, at: u32) -> vec4<f32> {
	let red_at = at + transfer_curve_length(at);
	let green_at = red_at + transfer_curve_length(red_at);
	let blue_at = green_at + transfer_curve_length(green_at);
	let alpha_at = blue_at + transfer_curve_length(blue_at);

	let red = clamp(evaluate_transfer_curve(at, clamp(evaluate_transfer_curve(red_at, color.r), 0., 1.)), 0., 1.);
	let green = clamp(evaluate_transfer_curve(at, clamp(evaluate_transfer_curve(green_at, color.g), 0., 1.)), 0., 1.);
	let blue = clamp(evaluate_transfer_curve(at, clamp(evaluate_transfer_curve(blue_at, color.b), 0., 1.)), 0., 1.);

	// Alpha stands apart from the composite curve that the three color channels pass through
	let alpha = clamp(evaluate_transfer_curve(alpha_at, color.a), 0., 1.);

	return vec4<f32>(red, green, blue, alpha);
}

// Whether to tint (x) and the reds, yellows, and greens weights (yzw), then the cyans, blues, and magentas weights (xyz), then the tint
fn black_and_white(color: vec4<f32>, at: u32) -> vec4<f32> {
	let weights = step_texel(at);
	let more_weights = step_texel(at + 1u);
	let reds = weights.y;
	let yellows = weights.z;
	let greens = weights.w;
	let cyans = more_weights.x;
	let blues = more_weights.y;
	let magentas = more_weights.z;

	let gray_base = minimum_channel(color.rgb);
	let red_part = color.r - gray_base;
	let green_part = color.g - gray_base;
	let blue_part = color.b - gray_base;

	// The smallest channel, which has no part past the gray, picks the pair of hues the others lie between
	var additional = 0.;
	if color.r <= gray_base {
		let cyan_part = min(green_part, blue_part);
		additional = cyan_part * cyans + (green_part - cyan_part) * greens + (blue_part - cyan_part) * blues;
	} else if color.g <= gray_base {
		let magenta_part = min(red_part, blue_part);
		additional = magenta_part * magentas + (red_part - magenta_part) * reds + (blue_part - magenta_part) * blues;
	} else {
		let yellow_part = min(red_part, green_part);
		additional = yellow_part * yellows + (red_part - yellow_part) * reds + (green_part - yellow_part) * greens;
	}

	let luminance = clamp(gray_base + additional, 0., 1.);
	if weights.x < 0.5 {
		return vec4<f32>(vec3<f32>(luminance), color.a);
	}

	// The tint takes on the gray's luminosity the way the Luminosity blend mode would
	let tint = to_gamma(step_texel(at + 2u)).rgb;
	return vec4<f32>(set_luminosity(tint, luma_rec_601_fixed_point(tint), luminance), color.a);
}

// HSL of gamma-encoded channels as hue in degrees, saturation, and lightness, matching `gamma_rgb_to_hsl`
fn gamma_rgb_to_hsl(rgb: vec3<f32>) -> vec3<f32> {
	let maximum = maximum_channel(rgb);
	let minimum = minimum_channel(rgb);
	let chroma = maximum - minimum;
	let lightness = (maximum + minimum) / 2.;
	if chroma <= 0. {
		return vec3<f32>(0., 0., lightness);
	}

	let saturation = chroma / max(1. - abs(2. * lightness - 1.), 1e-6);
	return vec3<f32>(hexagon_hue_degrees(rgb), min(saturation, 1.), lightness);
}

// Matches `hsl_to_gamma_rgb`
fn hsl_to_gamma_rgb(hue: f32, saturation: f32, lightness: f32) -> vec3<f32> {
	let chroma = (1. - abs(2. * lightness - 1.)) * saturation;
	let sector = wrap_positive(hue, 360.) / 60.;
	let x = chroma * (1. - abs(wrap_positive(sector, 2.) - 1.));

	var rgb = vec3<f32>(chroma, 0., x);
	if sector < 1. {
		rgb = vec3<f32>(chroma, x, 0.);
	} else if sector < 2. {
		rgb = vec3<f32>(x, chroma, 0.);
	} else if sector < 3. {
		rgb = vec3<f32>(0., chroma, x);
	} else if sector < 4. {
		rgb = vec3<f32>(0., x, chroma);
	} else if sector < 5. {
		rgb = vec3<f32>(x, 0., chroma);
	}

	return clamp(rgb + (lightness - chroma / 2.), vec3<f32>(0.), vec3<f32>(1.));
}

// Matches `lightness_toward_white_or_black`
fn lightness_toward_white_or_black(value: f32, amount: f32) -> f32 {
	if amount >= 0. {
		return value * (1. - amount) + amount;
	}
	return value * (1. + amount);
}

// Matches `lightness_toward_max_or_min`
fn lightness_toward_max_or_min(rgb: vec3<f32>, amount: f32) -> vec3<f32> {
	let toward = select(minimum_channel(rgb), maximum_channel(rgb), amount >= 0.);
	return rgb * (1. - abs(amount)) + toward * abs(amount);
}

fn hue_distance(start: f32, end: f32) -> f32 {
	return wrap_positive(end - start, 360.);
}

// The weight at a hue of a range from its falloff start (x), range start (y), range end (z), and falloff end (w), matching `HueSaturationRangeSettings::weight`
fn hue_range_weight(edges: vec4<f32>, hue: f32) -> f32 {
	if hue_distance(edges.y, hue) <= hue_distance(edges.y, edges.z) {
		return 1.;
	}

	let start_falloff = hue_distance(edges.x, edges.y);
	let end_falloff = hue_distance(edges.z, edges.w);
	if hue_distance(hue, edges.y) < start_falloff {
		return 1. - hue_distance(hue, edges.y) / start_falloff;
	}
	if hue_distance(edges.z, hue) < end_falloff {
		return 1. - hue_distance(edges.z, hue) / end_falloff;
	}
	return 0.;
}

// The six ranges' combined effect on one pixel, gathered before the master sliders apply, matching `HueSaturationRangeEffect`
struct HueSaturationRangeEffect {
	hue_shift: f32,
	saturation_factor: f32,
	fully_saturate: bool,
	rgb: vec3<f32>,
}

// The effect with a range added, from its edges and then its sliders, matching `HueSaturationRangeEffect::apply`
fn apply_hue_range(effect: HueSaturationRangeEffect, at: u32, original_hue: f32, original_saturation: f32) -> HueSaturationRangeEffect {
	let sliders = step_texel(at + 1u);

	// Range weights come from the original hue, and grays belong to no range
	var weight = 0.;
	if original_saturation > 0. {
		weight = hue_range_weight(step_texel(at), original_hue);
	}
	if weight <= 0. {
		return effect;
	}

	var result = effect;
	result.hue_shift += sliders.x * weight;
	// For PSD interop, +100 saturates fully from the very edge of the falloff rather than scaling with the weight
	if sliders.y >= 1. {
		result.fully_saturate = true;
	} else {
		result.saturation_factor *= 1. + (sliders.z - 1.) * weight;
	}
	result.rgb = lightness_toward_max_or_min(result.rgb, sliders.w * weight);

	return result;
}

// Whether to colorize (x), the master and colorize sliders, then the reds, yellows, greens, cyans, blues, and magentas ranges.
// Each set of sliders holds its hue (x), saturation (y), saturation gain (z), and lightness (w).
fn hue_saturation(color: vec4<f32>, at: u32) -> vec4<f32> {
	let master = step_texel(at + 1u);
	let colorize = step_texel(at + 2u);

	if step_texel(at).x > 0.5 {
		let lightness = lightness_toward_white_or_black(gamma_rgb_to_hsl(color.rgb).z, colorize.w);
		return vec4<f32>(hsl_to_gamma_rgb(colorize.x, colorize.y, lightness), color.a);
	}

	// Each range weights its sliders by its falloff around the original hue: hue shifts add and saturation gains multiply
	let original = gamma_rgb_to_hsl(color.rgb);
	var effect = HueSaturationRangeEffect(master.x, 1., false, color.rgb);
	for (var range_index = 0u; range_index < 6u; range_index++) {
		effect = apply_hue_range(effect, at + 3u + 2u * range_index, original.x, original.y);
	}
	if master.y >= 1. {
		effect.fully_saturate = true;
	} else {
		effect.saturation_factor *= master.z;
	}

	// The master lightness blends toward white or black before the hue and saturation, which work in HSL of the gamma channels
	let rgb = vec3<f32>(
		lightness_toward_white_or_black(effect.rgb.r, master.w),
		lightness_toward_white_or_black(effect.rgb.g, master.w),
		lightness_toward_white_or_black(effect.rgb.b, master.w)
	);
	let hsl = gamma_rgb_to_hsl(rgb);
	var saturation = min(hsl.y * effect.saturation_factor, 1.);
	if hsl.y <= 0. {
		saturation = 0.;
	} else if effect.fully_saturate {
		saturation = 1.;
	}

	return vec4<f32>(hsl_to_gamma_rgb(hsl.x + effect.hue_shift, saturation, hsl.z), color.a);
}

// The minimum (x) and maximum (y) luminance
fn threshold(color: vec4<f32>, at: u32) -> vec4<f32> {
	let bounds = step_texel(at);

	// For PSD interop, we compare this 14-bit fixed-point Rec. 601 luma against the level unrounded
	let luminance = luma_rec_601_fixed_point(color.rgb);
	let inside = luminance >= bounds.x - THRESHOLD_SLACK && luminance <= bounds.y + THRESHOLD_SLACK;

	return vec4<f32>(vec3<f32>(select(0., 1., inside)), color.a);
}

// How fully a hue falls under the skin-tone protection: all of it from red to 30 degrees, fading out by 45, and back in from 300
fn skin_tone_window(hue: f32) -> f32 {
	if hue < 45. {
		return min((45. - hue) / 15., 1.);
	} else if hue >= 300. {
		return (hue - 300.) / 60.;
	}
	return 0.;
}

// Share of the vibrance boost the skin-tone protection removes at full weight, a fitted constant
const VIBRANCE_PROTECTION_LOSS: f32 = 0.4857;

// Vibrance on linear channels as the chroma factor about the maximum channel (x) and the brightness multiplier (y), matching `vibrance_factors`
fn vibrance_factors(rgb: vec3<f32>, amount: f32) -> vec2<f32> {
	let maximum = maximum_channel(rgb);
	let minimum = minimum_channel(rgb);
	if maximum <= 0. {
		return vec2<f32>(1.);
	}

	let ratio = minimum / maximum;
	let saturation = 1. - ratio;
	let q = ratio * saturation;
	let toe = 1. - min(16. * maximum, 1.);
	let rolloff = 1. - toe * toe;
	let brightness = (2. * q - q * q) * (1. - maximum) * rolloff;

	if amount < 0. {
		let desaturation = -amount;
		let chroma_factor = (1. - desaturation / 4.) * (1. - desaturation * (1. - rolloff * saturation * (1. + saturation) / 2.));
		return vec2<f32>(chroma_factor, 1. - desaturation * brightness);
	}

	let protection = skin_tone_window(hexagon_hue_degrees(rgb)) * (1. - saturation * saturation);
	let protected_amount = amount * (1. - protection * (1. - amount));
	let boost = (5. / 6.) * (1. - VIBRANCE_PROTECTION_LOSS * protection) * protected_amount * ratio * (1. - minimum) * rolloff;
	return vec2<f32>(1. / (1. - boost), 1. + protected_amount * brightness / 4.);
}

// The vibrance amount (x) and saturation scale (y)
fn vibrance(color: vec4<f32>, at: u32) -> vec4<f32> {
	let settings = step_texel(at);

	// Vibrance then saturation, both in linear light, which equals applying each alone in turn
	let maximum = maximum_channel(color.rgb);
	let factors = vibrance_factors(color.rgb, settings.x);
	let after_vibrance = clamp(factors.y * (maximum + factors.x * (color.rgb - maximum)), vec3<f32>(0.), vec3<f32>(1.));

	// For PSD interop, saturation scales each channel's distance from a gray weighted by ProPhoto's luminance coefficients
	let gray = 0.288040 * after_vibrance.r + 0.711874 * after_vibrance.g + 0.000086 * after_vibrance.b;
	return vec4<f32>(clamp(gray + (after_vibrance - gray) * settings.y, vec3<f32>(0.), vec3<f32>(1.)), color.a);
}

// One output channel from the weights of the red (x), green (y), and blue (z) channels and a constant (w)
fn mix_channels(rgb: vec3<f32>, weights: vec4<f32>) -> f32 {
	return clamp(rgb.r * weights.x + rgb.g * weights.y + rgb.b * weights.z + weights.w, 0., 1.);
}

// Whether monochrome (x), then the monochrome, red, green, and blue weights
fn channel_mixer(color: vec4<f32>, at: u32) -> vec4<f32> {
	if step_texel(at).x > 0.5 {
		let gray = mix_channels(color.rgb, step_texel(at + 1u));
		return vec4<f32>(vec3<f32>(gray), color.a);
	}

	let red = mix_channels(color.rgb, step_texel(at + 2u));
	let green = mix_channels(color.rgb, step_texel(at + 3u));
	let blue = mix_channels(color.rgb, step_texel(at + 4u));
	return vec4<f32>(red, green, blue, color.a);
}

// Whether absolute (x) and the group count (y), then each group (x) with its inks for red, green, and blue (yzw)
fn selective_color(color: vec4<f32>, at: u32) -> vec4<f32> {
	let settings = step_texel(at);
	let rgb = color.rgb;

	let max_channel = maximum_channel(rgb);
	let min_channel = minimum_channel(rgb);
	let median = rgb.r + rgb.g + rgb.b - min_channel - max_channel;
	let color_parameter_group_scale_factor_rgb = max_channel - median;
	let color_parameter_group_scale_factor_cmy = median - min_channel;

	// Used to apply the r, g, or b channel slope (by multiplying it by 1) in relative mode, or no slope (by multiplying it by 0) in absolute mode
	var slope = rgb - 1.;
	if settings.x > 0.5 {
		slope = vec3<f32>(-1.);
	}

	var sum = vec3<f32>(0.);
	for (var index = 0u; index < u32(settings.y); index++) {
		let group_inks = step_texel(at + 1u + index);

		// Skip this group if this pixel's color isn't in the range it affects
		var in_range = true;
		var color_parameter_group_scale_factor = color_parameter_group_scale_factor_rgb;
		switch u32(group_inks.x) {
			case REDS: {
				in_range = rgb.r >= max_channel;
			}
			case YELLOWS: {
				in_range = rgb.b <= min_channel;
				color_parameter_group_scale_factor = color_parameter_group_scale_factor_cmy;
			}
			case GREENS: {
				in_range = rgb.g >= max_channel;
			}
			case CYANS: {
				in_range = rgb.r <= min_channel;
				color_parameter_group_scale_factor = color_parameter_group_scale_factor_cmy;
			}
			case BLUES: {
				in_range = rgb.b >= max_channel;
			}
			case MAGENTAS: {
				in_range = rgb.g <= min_channel;
				color_parameter_group_scale_factor = color_parameter_group_scale_factor_cmy;
			}
			case WHITES: {
				in_range = rgb.r > 0.5 && rgb.g > 0.5 && rgb.b > 0.5;
				color_parameter_group_scale_factor = min_channel * 2. - 1.;
			}
			case NEUTRALS: {
				// Every pixel, since the neutrals scale factor already vanishes at black, white, and fully saturated colors
				color_parameter_group_scale_factor = 1. - (abs(max_channel - 0.5) + abs(min_channel - 0.5));
			}
			case BLACKS, default: {
				in_range = rgb.r < 0.5 && rgb.g < 0.5 && rgb.b < 0.5;
				color_parameter_group_scale_factor = 1. - max_channel * 2.;
			}
		}
		if !in_range {
			continue;
		}

		sum += min(max(group_inks.yzw * slope, -rgb), -rgb + 1.) * color_parameter_group_scale_factor;
	}

	return vec4<f32>(clamp(sum + rgb, vec3<f32>(0.), vec3<f32>(1.)), color.a);
}

// The level count (x)
fn posterize(color: vec4<f32>, at: u32) -> vec4<f32> {
	let levels = step_texel(at).x;

	// Bins as floor(c * levels) with the outputs spread evenly to white.
	// The sliver of slack keeps an input exactly on an edge in the upper bin despite float ties.
	let bins = min(floor((color.rgb + 2e-7) * levels), vec3<f32>(levels - 1.));
	return vec4<f32>(min(bins / (levels - 1.), vec3<f32>(1.)), color.a);
}

// Linearizes with a 2.2 power above a straight toe of slope 1/32, the two meeting at this constant
const EXPOSURE_TOE_END: f32 = 0.05568117;

fn exposure_decode(value: f32) -> f32 {
	if value < EXPOSURE_TOE_END {
		return value / 32.;
	}
	return pow(value, 2.2);
}

fn exposure_encode(linear: f32) -> f32 {
	if linear < EXPOSURE_TOE_END / 32. {
		return linear * 32.;
	}
	return pow(linear, 1. / 2.2);
}

fn exposure_channel(value: f32, settings: vec4<f32>) -> f32 {
	let linear = exposure_decode(value) * exp2(settings.x) + settings.y;
	return min(exposure_encode(min(rust_powf(max(linear, 0.), 1. / settings.z), 1.)), 1.);
}

// The exposure (x), offset (y), and gamma correction (z)
fn exposure(color: vec4<f32>, at: u32) -> vec4<f32> {
	let settings = step_texel(at);
	return vec4<f32>(exposure_channel(color.r, settings), exposure_channel(color.g, settings), exposure_channel(color.b, settings), color.a);
}

// The Levels curves of the red, green, and blue channels
fn color_balance(color: vec4<f32>, at: u32) -> vec4<f32> {
	let red = apply_levels_curve(at, color.r);
	let green = apply_levels_curve(at + LEVELS_CURVE_TEXELS, color.g);
	let blue = apply_levels_curve(at + 2u * LEVELS_CURVE_TEXELS, color.b);
	return vec4<f32>(red, green, blue, color.a);
}

// The XYZ factors (xyz) and whether to preserve luminosity (w)
fn photo_filter(color: vec4<f32>, at: u32) -> vec4<f32> {
	let settings = step_texel(at);

	let xyz = srgb_to_xyz_d50(srgb_to_linear_rgb(color.rgb));
	let filtered = clamp(xyz_d50_to_srgb(xyz * settings.xyz), vec3<f32>(0.), vec3<f32>(1.));
	var rgb = min(vec3<f32>(linear_to_srgb(filtered.r), linear_to_srgb(filtered.g), linear_to_srgb(filtered.b)), vec3<f32>(1.));

	if settings.w > 0.5 {
		let luma_in = luma_rec_601_fixed_point(clamp(color.rgb, vec3<f32>(0.), vec3<f32>(1.)));
		rgb = set_luminosity(rgb, luma_rec_601_fixed_point(rgb), luma_in);
	}

	return vec4<f32>(rgb, color.a);
}

// ============
// Gradient Map
// ============

// The sRGB transfer function of the `color` crate, which unlike `srgb_to_linear` mirrors itself for negative values
fn color_crate_srgb_to_linear(channel: f32) -> f32 {
	if abs(channel) <= 0.04045 {
		return channel * (1. / 12.92);
	}
	return sign(channel) * pow((abs(channel) + 0.055) * (1. / 1.055), 2.4);
}

fn color_crate_srgb_to_linear_rgb(rgb: vec3<f32>) -> vec3<f32> {
	return vec3<f32>(color_crate_srgb_to_linear(rgb.r), color_crate_srgb_to_linear(rgb.g), color_crate_srgb_to_linear(rgb.b));
}

// Matches `Oklab::to_linear_srgb`
fn oklab_to_linear_srgb(lab: vec3<f32>) -> vec3<f32> {
	let lms = vec3<f32>(
		lab.x + 0.39633778 * lab.y + 0.21580376 * lab.z,
		lab.x - 0.105561346 * lab.y - 0.06385417 * lab.z,
		lab.x - 0.08948418 * lab.y - 1.2914855 * lab.z
	);
	let cubed = lms * lms * lms;

	return vec3<f32>(
		4.0767417 * cubed.x - 3.3077116 * cubed.y + 0.23096994 * cubed.z,
		-1.268438 * cubed.x + 2.6097574 * cubed.y - 0.34131938 * cubed.z,
		-0.0041960863 * cubed.x - 0.7034186 * cubed.y + 1.7076147 * cubed.z
	);
}

const LAB_KAPPA: f32 = 24389. / 27.;

fn lab_f_to_xyz(value: f32) -> f32 {
	// The cube root of the color crate's `EPSILON`
	if value > 0.20689656 {
		return value * value * value;
	}
	return (116. / LAB_KAPPA) * value - (16. / LAB_KAPPA);
}

// Matches `Lab::to_linear_srgb`
fn lab_to_linear_srgb(lab: vec3<f32>) -> vec3<f32> {
	let f1 = lab.x * (1. / 116.) + (16. / 116.);
	let f0 = lab.y * (1. / 500.) + f1;
	let f2 = f1 - lab.z * (1. / 200.);
	let xyz = vec3<f32>(lab_f_to_xyz(f0), lab_f_to_xyz(f1), lab_f_to_xyz(f2));

	return vec3<f32>(
		3.0222337 * xyz.x - 1.617386 * xyz.y - 0.40484765 * xyz.z,
		-0.94384825 * xyz.x + 1.9162544 * xyz.y + 0.027593868 * xyz.z,
		0.06938627 * xyz.x - 0.22897676 * xyz.y + 1.1595905 * xyz.z
	);
}

// Matches `lch_to_lab`, after turning the hue within a half turn of zero, where the GPU's sine and cosine are most precise
fn lch_to_lab(lch: vec3<f32>) -> vec3<f32> {
	let hue = radians(lch.z - 360. * floor(lch.z / 360. + 0.5));
	return vec3<f32>(lch.x, lch.y * cos(hue), lch.y * sin(hue));
}

fn hsl_channel(offset: f32, hue: f32, lightness: f32, a: f32) -> f32 {
	let x = offset + hue * (1. / 30.);
	let k = x - 12. * floor(x * (1. / 12.));
	return lightness - a * clamp(min(k - 3., 9. - k), -1., 1.);
}

// Matches `hsl_to_rgb` of the `color` crate, for HSL in degrees and percentages
fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
	let saturation = hsl.y * 0.01;
	let lightness = hsl.z * 0.01;
	let a = saturation * min(lightness, 1. - lightness);

	return vec3<f32>(hsl_channel(0., hsl.x, lightness, a), hsl_channel(8., hsl.x, lightness, a), hsl_channel(4., hsl.x, lightness, a));
}

fn hsv_channel(offset: f32, hue: f32, saturation: f32, value: f32) -> f32 {
	let k = wrap_positive(offset + hue / 60., 6.);
	return value - value * saturation * clamp(min(k, 4. - k), 0., 1.);
}

// Matches `Hsv::to_linear_srgb`, for HSV in degrees and percentages
fn hsv_to_linear_srgb(hsv: vec3<f32>) -> vec3<f32> {
	let saturation = hsv.y / 100.;
	let value = hsv.z / 100.;

	let rgb = vec3<f32>(hsv_channel(5., hsv.x, saturation, value), hsv_channel(3., hsv.x, saturation, value), hsv_channel(1., hsv.x, saturation, value));
	return color_crate_srgb_to_linear_rgb(rgb);
}

// Matches `color_from_space_channels` for each space's `ColorSpace::to_linear_srgb`
fn space_to_linear_srgb(space: u32, channels: vec3<f32>) -> vec3<f32> {
	switch space {
		case OKLAB: {
			return oklab_to_linear_srgb(channels);
		}
		case LAB: {
			return lab_to_linear_srgb(channels);
		}
		case OKLCH: {
			return oklab_to_linear_srgb(lch_to_lab(channels));
		}
		case LCH: {
			return lab_to_linear_srgb(lch_to_lab(channels));
		}
		case RGB_LINEAR: {
			return channels;
		}
		case RGB_GAMMA: {
			return color_crate_srgb_to_linear_rgb(channels);
		}
		case RGB_GAMMA_MIXED: {
			return srgb_to_linear_rgb(channels);
		}
		case HSV: {
			return hsv_to_linear_srgb(channels);
		}
		case HSL, default: {
			return color_crate_srgb_to_linear_rgb(hsl_to_rgb(channels));
		}
	}
}

// Stands in for RGB_GAMMA where two stops' channels were mixed, which `Color::lerp_gamma_srgb` decodes as `srgb_to_linear` does
const RGB_GAMMA_MIXED: u32 = 255u;

// A ramp's color as channels in a space. Each way of evaluating the ramp gives one for `gradient_map` to convert,
// so the conversion (like each of the ramp's other long functions) is called from one place and compiled once.
struct RampSample {
	channels: vec3<f32>,
	space: u32,
}

// Matches `apply_midpoint` for a midpoint already sanitized, held at or above 0 since the GPU's division can land a hair past 1
fn apply_midpoint(t: f32, midpoint: f32) -> f32 {
	if abs(midpoint - 0.5) < 1e-6 {
		return t;
	}

	if midpoint < 0.5 {
		let q = -1. / log2(1. - midpoint);
		return 1. - rust_powf(max(1. - t, 0.), q);
	}
	let p = -1. / log2(midpoint);
	return rust_powf(max(t, 0.), p);
}

// After the stop count (x) and whether cyclic (y), each stop's position (paired in xy, as in `at_or_past`), then its color, matching `stepped_color`
fn stepped_gradient_color(at: u32, t: f32) -> vec3<f32> {
	let settings = step_texel(at);
	let count = u32(settings.x);
	if count == 0u {
		return vec3<f32>(0.);
	}

	// Before the first stop a cyclic ramp is still inside the wrapped interval, which the last stop's color holds
	let first = step_texel(at + 1u).xy;
	let last = step_texel(at + 2u * count - 1u).xy;
	let last_color = step_texel(at + 2u * count).xyz;
	if !at_or_past(t, first) {
		return select(step_texel(at + 2u).xyz, last_color, settings.y > 0.5);
	}
	if at_or_past(t, last) {
		return last_color;
	}

	for (var index = 0u; index + 1u < count; index++) {
		if !at_or_past(t, step_texel(at + 3u + 2u * index).xy) {
			return step_texel(at + 2u + 2u * index).xyz;
		}
	}
	return last_color;
}

// After the stop count (x) and whether cyclic (y), each stop's position (paired in xy, as in `at_or_past`), the width of its interval to the next stop
// (the last stop's to the first) in double precision (z), and its midpoint (w), then its color, then the channels its interval interpolates between, matching `linear_color`
fn linear_gradient_sample(at: u32, space: u32, t: f32) -> RampSample {
	let settings = step_texel(at);
	let count = u32(settings.x);
	if count == 0u {
		return RampSample(vec3<f32>(0.), RGB_LINEAR);
	}
	let first_at = at + 1u;
	let last_at = at + 1u + 4u * (count - 1u);
	let first = step_texel(first_at);
	let last = step_texel(last_at);

	// The interval holding the position, from the stop at `interval_at` to the next, and how far along it the position lies
	var interval_at = last_at;
	var local = 0.;
	if settings.y > 0.5 && (!at_or_past(t, first.xy) || !at_or_before(t, last.xy)) {
		// A cyclic ramp's wrapped interval, from the last stop around to the first
		let wrap_length = last.z;
		if wrap_length <= F64_EPSILON {
			return RampSample(step_texel(first_at + 1u).xyz, RGB_LINEAR);
		}

		local = (t + (1. - last.x)) - last.y;
		if at_or_past(t, last.xy) {
			local = distance_past(t, last.xy);
		}
		local /= wrap_length;
	} else {
		if at_or_before(t, first.xy) {
			return RampSample(step_texel(first_at + 1u).xyz, RGB_LINEAR);
		}
		if at_or_past(t, last.xy) {
			return RampSample(step_texel(last_at + 1u).xyz, RGB_LINEAR);
		}

		var found = false;
		for (var index = 0u; index + 1u < count; index++) {
			let stop_at = at + 1u + 4u * index;
			let start = step_texel(stop_at);
			if at_or_past(t, start.xy) && at_or_before(t, step_texel(stop_at + 4u).xy) {
				interval_at = stop_at;
				local = distance_past(t, start.xy) / start.z;
				found = true;
				break;
			}
		}
		if !found {
			return RampSample(vec3<f32>(0.), RGB_LINEAR);
		}
	}

	// Matches `interpolate_stop_colors` from the channels `interpolation_endpoints` gives
	let adjusted_t = apply_midpoint(local, step_texel(interval_at).w);
	let start = step_texel(interval_at + 2u).xyz;
	let end = step_texel(interval_at + 3u).xyz;
	return RampSample(start + (end - start) * adjusted_t, select(space, RGB_GAMMA_MIXED, space == RGB_GAMMA));
}

// Up to three monotonic splines sharing their knots, matching `MonotonicSpline::evaluate`. Each knot holds its position (paired in xy, as in `at_or_past`)
// and the run to the next in double precision (z), then its value in each spline, then its tangent in each.
fn evaluate_monotonic_splines(at: u32, count: u32, position: f32) -> vec3<f32> {
	if count == 0u {
		return vec3<f32>(0.);
	}
	if count == 1u || at_or_before(position, step_texel(at).xy) {
		return step_texel(at + 1u).xyz;
	}
	let last_at = at + 3u * (count - 1u);
	if at_or_past(position, step_texel(last_at).xy) {
		return step_texel(last_at + 1u).xyz;
	}

	let start_at = at + 3u * (clamp(count_at_or_below(at, 3u, count, position), 1u, count - 1u) - 1u);
	let start = step_texel(start_at);
	let end_values = step_texel(start_at + 4u).xyz;
	let run = start.z;
	if abs(run) < F64_EPSILON {
		return end_values;
	}
	let start_values = step_texel(start_at + 1u).xyz;
	let start_tangents = step_texel(start_at + 2u).xyz;
	let end_tangents = step_texel(start_at + 5u).xyz;

	let t = distance_past(position, start.xy) / run;
	let t2 = t * t;
	let t3 = t * t * t;
	return (2. * t3 - 3. * t2 + 1.) * start_values + (t3 - 2. * t2 + t) * run * start_tangents + (-2. * t3 + 3. * t2) * end_values + (t3 - t2) * run * end_tangents;
}

// The knot counts of the warp (x) and the channel splines (y), then the warp's knots and the channel splines' knots, matching `SmoothPath::evaluate`
fn smooth_gradient_sample(at: u32, space: u32, t: f32) -> RampSample {
	let counts = step_texel(at);
	let warp_count = u32(counts.x);
	let parameter = evaluate_monotonic_splines(at + 1u, warp_count, t).x;
	var channels = evaluate_monotonic_splines(at + 1u + 3u * warp_count, u32(counts.y), parameter);

	// The hue channel wraps into a turn
	if space == OKLCH || space == LCH {
		channels.z = wrap_positive(channels.z, 360.);
	} else if space == HSV || space == HSL {
		channels.x = wrap_positive(channels.x, 360.);
	}

	return RampSample(channels, space);
}

// Whether reversed (x), the spread (y), how the ramp is evaluated (z), and its space (w), then the ramp
fn gradient_map(color: vec4<f32>, at: u32) -> vec4<f32> {
	let settings = step_texel(at);
	let space = u32(settings.w);

	// The classic 0.3/0.59/0.11 luma of the gamma-encoded channels picks the position along the gradient
	var t = luma_rec_601_rounded(color.rgb);
	if settings.x > 0.5 {
		t = 1. - t;
	}

	// A position a hair past either end lies on it, matching `GradientEvaluator::evaluate_mapped` here and in what follows
	if t > 1. && t <= 1. + MAPPED_POSITION_SLACK {
		t = 1.;
	} else if t < 0. && t >= -MAPPED_POSITION_SLACK {
		t = 0.;
	}

	// The spread decides where a position past either end lands
	switch u32(settings.y) {
		case PAD: {
			t = clamp(t, 0., 1.);
		}
		case REFLECT: {
			let cycle = wrap_positive(t, 2.);
			t = select(cycle, 2. - cycle, cycle > 1.);
		}
		case REPEAT: {
			// The ramp's end stays its end, so white maps to the last color
			let wrapped = wrap_positive(t, 1.);
			t = select(wrapped, 1., t > 0. && wrapped == 0.);
		}
		case CLEAR, default: {
			if !(t >= 0. && t <= 1.) {
				return vec4<f32>(0., 0., 0., color.a);
			}
		}
	}

	// Each stop of a stepped ramp reaches a hair short of itself
	var sample = RampSample(vec3<f32>(0.), RGB_LINEAR);
	switch u32(settings.z) {
		case STEPPED: {
			sample.channels = stepped_gradient_color(at + 1u, t + MAPPED_POSITION_SLACK);
		}
		case SMOOTH: {
			sample = smooth_gradient_sample(at + 1u, space, t);
		}
		case LINEAR, default: {
			sample = linear_gradient_sample(at + 1u, space, t);
		}
	}

	// The source alpha is kept and the gradient's own alpha stops are ignored
	return vec4<f32>(space_to_linear_srgb(sample.space, sample.channels), color.a);
}

// ============
// Color Lookup
// ============

// A table's blocks each start with a header holding the block's kind (x), a setting (y), and how many texels it spans (as two 16-bit halves in zw)

// Matches `LabEncoding::encode`
fn lab_encode(encoding: u32, lab: vec3<f32>) -> vec3<f32> {
	if encoding == LAB_STANDARD {
		return vec3<f32>(lab.x / 100., (lab.y + 128.) / 255., (lab.z + 128.) / 255.);
	}
	return vec3<f32>(lab.x / 100. * (65280. / 65535.), (lab.y + 128.) * (256. / 65535.), (lab.z + 128.) * (256. / 65535.));
}

// Matches `LabEncoding::decode`
fn lab_decode(encoding: u32, encoded: vec3<f32>) -> vec3<f32> {
	if encoding == LAB_STANDARD {
		return vec3<f32>(encoded.x * 100., encoded.y * 255. - 128., encoded.z * 255. - 128.);
	}
	return vec3<f32>(encoded.x * 100. * (65535. / 65280.), encoded.y * (65535. / 256.) - 128., encoded.z * (65535. / 256.) - 128.);
}

fn lab_f(t: f32) -> f32 {
	if t > 216. / 24389. {
		return cube_root(t);
	}
	return (24389. / 27. * t + 16.) / 116.;
}

fn lab_f_inverse(t: f32) -> f32 {
	if t > 6. / 29. {
		return t * t * t;
	}
	return 3. * (6. / 29.) * (6. / 29.) * (t - 4. / 29.);
}

// Gamma-encoded sRGB to CIELAB against the D50 white of the ICC profile connection space, matching `srgb_to_lab`
fn srgb_to_lab(rgb: vec3<f32>) -> vec3<f32> {
	let xyz = srgb_to_xyz_d50(srgb_to_linear_rgb(rgb)) / WHITE_XYZ_D50;
	let f = vec3<f32>(lab_f(xyz.x), lab_f(xyz.y), lab_f(xyz.z));
	return vec3<f32>(116. * f.y - 16., 500. * (f.x - f.y), 200. * (f.y - f.z));
}

// Matches `lab_to_srgb`
fn lab_to_srgb(lab: vec3<f32>) -> vec3<f32> {
	let fy = (lab.x + 16.) / 116.;
	let fx = fy + lab.y / 500.;
	let fz = fy - lab.z / 200.;
	let xyz = vec3<f32>(lab_f_inverse(fx), lab_f_inverse(fy), lab_f_inverse(fz)) * WHITE_XYZ_D50;

	let rgb = clamp(xyz_d50_to_srgb(xyz), vec3<f32>(0.), vec3<f32>(1.));
	return min(vec3<f32>(linear_to_srgb(rgb.r), linear_to_srgb(rgb.g), linear_to_srgb(rgb.b)), vec3<f32>(1.));
}

// Matches `parametric_curve` from the parameters g, a, b, and c (xyzw) and d, e, and f (xyz)
fn parametric_curve(curve_function: u32, first: vec4<f32>, second: vec4<f32>, x: f32) -> f32 {
	let g = first.x;
	let a = first.y;
	let b = first.z;
	let c = first.w;
	let d = second.x;
	let e = second.y;
	let f = second.z;
	let power = rust_powf(max(a * x + b, 0.), g);

	// The breakpoint -b / a of functions 1 and 2, which with an `a` of 0 lies at an infinity on the side opposite the sign of `b`, or nowhere for a `b` of 0
	var past_breakpoint = b > 0.;
	if a != 0. {
		past_breakpoint = x >= -b / a;
	}

	var result = x;
	switch curve_function {
		case 0u: {
			result = rust_powf(max(x, 0.), g);
		}
		case 1u: {
			result = select(0., power, past_breakpoint);
		}
		case 2u: {
			result = select(c, power + c, past_breakpoint);
		}
		case 3u: {
			result = select(c * x, power, x >= d);
		}
		case 4u: {
			result = select(c * x + f, power + e, x >= d);
		}
		default: {}
	}

	// The specification clips every function to the unit range
	return clamp(result, 0., 1.);
}

// A curve block's mapping of a value, matching `Curve::apply`
fn apply_lut_curve(at: u32, value: f32) -> f32 {
	let header = table_texel(at);
	let count = block_length(at) - 1u;

	switch u32(header.x) {
		case SAMPLED_CURVE: {
			if count == 0u {
				return value;
			}
			if count == 1u {
				return table_texel(at + 1u).x;
			}

			let last = count - 1u;
			let scaled = clamp(value, 0., 1.) * f32(last);
			let lower = min(u32(floor(scaled)), last);
			let upper = min(lower + 1u, last);
			let lower_sample = table_texel(at + 1u + lower).x;
			return lower_sample + (scaled - f32(lower)) * (table_texel(at + 1u + upper).x - lower_sample);
		}
		case PIECEWISE_CURVE: {
			if count == 0u {
				return value;
			}
			let first = table_texel(at + 1u);
			if value <= first.x {
				return first.y;
			}
			let last = table_texel(at + count);
			if value >= last.x {
				return last.y;
			}

			// A NaN value leaves the search without a segment, so the clamp keeps it in bounds
			let upper = clamp(table_count_at_or_below(at + 1u, count, value), 1u, count - 1u);
			let lower_point = table_texel(at + upper);
			let upper_point = table_texel(at + 1u + upper);
			let t = (value - lower_point.x) / max(upper_point.x - lower_point.x, F32_EPSILON);
			return lower_point.y + t * (upper_point.y - lower_point.y);
		}
		case POWER_CURVE: {
			return rust_powf(max(value, 0.), header.y);
		}
		case PARAMETRIC_CURVE, default: {
			return parametric_curve(u32(header.y), table_texel(at + 1u), table_texel(at + 2u), value);
		}
	}
}

// How many texels the block of a table at `at` spans, header included
fn block_length(at: u32) -> u32 {
	return joined_count(table_texel(at).zw);
}

// The entry at a point of a three-dimensional table, stored with red varying fastest or else blue
fn lut_entry(entries: u32, size: vec3<u32>, red_fastest: bool, point: vec3<u32>) -> vec3<f32> {
	var index = (point.r * size.y + point.g) * size.z + point.b;
	if red_fastest {
		index = (point.b * size.y + point.g) * size.x + point.r;
	}
	return table_texel(entries + index).xyz;
}

// The entries of a one-dimensional table interpolated at a value from 0 to 1, of which each channel reads its own
fn one_dimensional_lookup(entries: u32, last: u32, value: f32) -> vec3<f32> {
	let scaled = value * f32(last);
	let lower = min(u32(floor(scaled)), last);
	let upper = min(lower + 1u, last);

	let lower_entry = table_texel(entries + lower).xyz;
	return lower_entry + (scaled - f32(lower)) * (table_texel(entries + upper).xyz - lower_entry);
}

// A three-dimensional table's tetrahedral interpolation, walking from the cell's origin to its far corner along the axes in decreasing fraction order
fn three_dimensional_lookup(at: u32, normalized: vec3<f32>) -> vec3<f32> {
	let size = vec3<u32>(table_texel(at + 1u).xyz);
	let red_fastest = table_texel(at).y > 0.5;
	let entries = at + 2u;

	let scaled = normalized * vec3<f32>(size - 1u);
	let lower = min(vec3<u32>(floor(scaled)), size - 1u);
	let upper = min(lower + 1u, size - 1u);
	let within = scaled - vec3<f32>(lower);

	// The axes the walk steps along first and second and the fractions in its order, picked first so only the four corners on the walk are read
	var first_axis = vec3<u32>(0u, 0u, 1u);
	var second_axis = vec3<u32>(0u, 1u, 0u);
	var fractions = within.bgr;
	if within.r >= within.g && within.g >= within.b {
		first_axis = vec3<u32>(1u, 0u, 0u);
		second_axis = vec3<u32>(0u, 1u, 0u);
		fractions = within.rgb;
	} else if within.r >= within.b && within.b >= within.g {
		first_axis = vec3<u32>(1u, 0u, 0u);
		second_axis = vec3<u32>(0u, 0u, 1u);
		fractions = within.rbg;
	} else if within.b >= within.r && within.r >= within.g {
		first_axis = vec3<u32>(0u, 0u, 1u);
		second_axis = vec3<u32>(1u, 0u, 0u);
		fractions = within.brg;
	} else if within.g >= within.r && within.r >= within.b {
		first_axis = vec3<u32>(0u, 1u, 0u);
		second_axis = vec3<u32>(1u, 0u, 0u);
		fractions = within.grb;
	} else if within.g >= within.b && within.b >= within.r {
		first_axis = vec3<u32>(0u, 1u, 0u);
		second_axis = vec3<u32>(0u, 0u, 1u);
		fractions = within.gbr;
	}

	// Each corner steps from the origin by the cell's extent (1, or 0 at the table's edge) along the axes walked so far
	let extent = upper - lower;
	let origin = lut_entry(entries, size, red_fastest, lower);
	let first = lut_entry(entries, size, red_fastest, lower + extent * first_axis);
	let second = lut_entry(entries, size, red_fastest, lower + extent * (first_axis + second_axis));
	let far = lut_entry(entries, size, red_fastest, upper);
	return origin + (fractions.x * (first - origin) + fractions.y * (second - first) + fractions.z * (far - second));
}

// A table block's mapping of normalized channels, matching the table lookup of `Lut::apply`
fn apply_lut_table(at: u32, normalized: vec3<f32>) -> vec3<f32> {
	switch u32(table_texel(at).x) {
		case ONE_DIMENSIONAL_TABLE: {
			// One table per channel, each channel looked up by its own value
			let last = block_length(at) - 2u;
			let red = one_dimensional_lookup(at + 1u, last, normalized.r).r;
			let green = one_dimensional_lookup(at + 1u, last, normalized.g).g;
			let blue = one_dimensional_lookup(at + 1u, last, normalized.b).b;
			return vec3<f32>(red, green, blue);
		}
		case THREE_DIMENSIONAL_TABLE: {
			return three_dimensional_lookup(at, normalized);
		}
		case NO_TABLE, default: {
			return normalized;
		}
	}
}

// A matrix block's mapping, each of its rows with its offset (w), matching `Stage::apply`
fn apply_lut_matrix(at: u32, values: vec3<f32>) -> vec3<f32> {
	let first = table_texel(at + 1u);
	let second = table_texel(at + 2u);
	let third = table_texel(at + 3u);
	return vec3<f32>(
		first.x * values.x + first.y * values.y + first.z * values.z + first.w,
		second.x * values.x + second.y * values.y + second.z * values.z + second.w,
		third.x * values.x + third.y * values.y + third.z * values.z + third.w
	);
}

// The table's number among the run's tables (x), matching `Lut::apply`. The table holds the Lab encoding (x) and how many blocks follow (y), then the domain's
// minimum and maximum, then its blocks in the order they map the color: the curves of each channel in turn, the table, and the matrices.
fn color_lookup(color: vec4<f32>, at: u32) -> vec4<f32> {
	let lut = table_start(u32(step_texel(at).x));
	let settings = table_texel(lut);
	let lab_encoding = u32(settings.x);

	var values = color.rgb;
	if lab_encoding != LAB_NONE {
		values = lab_encode(lab_encoding, srgb_to_lab(color.rgb));
	}

	// One loop carries out every block, so the code of each kind is called from one place and compiled once however many blocks a table holds
	var block_at = lut + 3u;
	var channel = 0u;
	for (var index = 0u; index < u32(settings.y); index++) {
		switch u32(table_texel(block_at).x) {
			case SAMPLED_CURVE, PIECEWISE_CURVE, POWER_CURVE, PARAMETRIC_CURVE: {
				// Curves come in threes, one for each channel in turn, which is picked without indexing (FXC can't write to an index in a loop)
				let own = vec3<u32>(0u, 1u, 2u) == vec3<u32>(channel);
				let mapped = apply_lut_curve(block_at, select(select(values.b, values.g, own.g), values.r, own.r));
				values = select(values, vec3<f32>(mapped), own);
				channel = (channel + 1u) % 3u;
			}
			case MATRIX_STAGE: {
				values = apply_lut_matrix(block_at, values);
			}
			case NO_TABLE, ONE_DIMENSIONAL_TABLE, THREE_DIMENSIONAL_TABLE, default: {
				let domain_min = table_texel(lut + 1u).xyz;
				let span = max(table_texel(lut + 2u).xyz - domain_min, vec3<f32>(F32_EPSILON));
				values = apply_lut_table(block_at, clamp((values - domain_min) / span, vec3<f32>(0.), vec3<f32>(1.)));
			}
		}
		block_at += block_length(block_at);
	}

	if lab_encoding != LAB_NONE {
		values = lab_to_srgb(lab_decode(lab_encoding, values));
	}
	return vec4<f32>(values, color.a);
}

// =============
// Color Overlay
// =============

// The per-channel blend functions of `Color`, each of the backdrop's channel and the source's

fn blend_multiply(c_b: f32, c_s: f32) -> f32 {
	return c_s * c_b;
}

fn blend_darken(c_b: f32, c_s: f32) -> f32 {
	return min(c_s, c_b);
}

fn blend_color_burn(c_b: f32, c_s: f32) -> f32 {
	if abs(c_b - 1.) <= BLEND_EDGE_TOLERANCE {
		return 1.;
	} else if abs(c_s) <= BLEND_EDGE_TOLERANCE {
		return 0.;
	}
	return 1. - min((1. - c_b) / c_s, 1.);
}

fn blend_linear_burn(c_b: f32, c_s: f32) -> f32 {
	return c_b + c_s - 1.;
}

fn blend_screen(c_b: f32, c_s: f32) -> f32 {
	return 1. - (1. - c_s) * (1. - c_b);
}

fn blend_lighten(c_b: f32, c_s: f32) -> f32 {
	return max(c_s, c_b);
}

fn blend_color_dodge(c_b: f32, c_s: f32) -> f32 {
	if abs(c_s - 1.) <= BLEND_EDGE_TOLERANCE {
		return 1.;
	}
	return min(c_b / (1. - c_s), 1.);
}

fn blend_linear_dodge(c_b: f32, c_s: f32) -> f32 {
	return c_b + c_s;
}

fn blend_softlight(c_b: f32, c_s: f32) -> f32 {
	if c_s <= 0.5 {
		return c_b - (1. - 2. * c_s) * c_b * (1. - c_b);
	}

	var d = sqrt(c_b);
	if c_b <= 0.25 {
		d = ((16. * c_b - 12.) * c_b + 4.) * c_b;
	}
	return c_b + (2. * c_s - 1.) * (d - c_b);
}

fn blend_hardlight(c_b: f32, c_s: f32) -> f32 {
	if c_s <= 0.5 {
		return blend_multiply(2. * c_s, c_b);
	}
	return blend_screen(2. * c_s - 1., c_b);
}

fn blend_vivid_light(c_b: f32, c_s: f32) -> f32 {
	if c_s <= 0.5 {
		return blend_color_burn(c_b, 2. * c_s);
	}
	return blend_color_dodge(c_b, 2. * c_s - 1.);
}

fn blend_linear_light(c_b: f32, c_s: f32) -> f32 {
	if c_s <= 0.5 {
		return blend_linear_burn(2. * c_s, c_b);
	}
	return blend_linear_dodge(2. * c_s - 1., c_b);
}

fn blend_pin_light(c_b: f32, c_s: f32) -> f32 {
	if c_s <= 0.5 {
		return blend_darken(2. * c_s, c_b);
	}
	return blend_lighten(2. * c_s - 1., c_b);
}

fn blend_channel(mode: u32, c_b: f32, c_s: f32) -> f32 {
	switch mode {
		case BLEND_DARKEN: {
			return blend_darken(c_b, c_s);
		}
		case BLEND_MULTIPLY: {
			return blend_multiply(c_b, c_s);
		}
		case BLEND_COLOR_BURN: {
			return blend_color_burn(c_b, c_s);
		}
		case BLEND_LINEAR_BURN: {
			return blend_linear_burn(c_b, c_s);
		}
		case BLEND_LIGHTEN: {
			return blend_lighten(c_b, c_s);
		}
		case BLEND_SCREEN: {
			return blend_screen(c_b, c_s);
		}
		case BLEND_COLOR_DODGE: {
			return blend_color_dodge(c_b, c_s);
		}
		case BLEND_LINEAR_DODGE: {
			return blend_linear_dodge(c_b, c_s);
		}
		case BLEND_OVERLAY: {
			return blend_hardlight(c_s, c_b);
		}
		case BLEND_SOFT_LIGHT: {
			return blend_softlight(c_b, c_s);
		}
		case BLEND_HARD_LIGHT: {
			return blend_hardlight(c_b, c_s);
		}
		case BLEND_VIVID_LIGHT: {
			return blend_vivid_light(c_b, c_s);
		}
		case BLEND_LINEAR_LIGHT: {
			return blend_linear_light(c_b, c_s);
		}
		case BLEND_PIN_LIGHT: {
			return blend_pin_light(c_b, c_s);
		}
		case BLEND_HARD_MIX: {
			return select(1., 0., blend_linear_light(c_b, c_s) < 0.5);
		}
		case BLEND_DIFFERENCE: {
			return abs(c_b - c_s);
		}
		case BLEND_EXCLUSION: {
			return c_b + c_s - 2. * c_b * c_s;
		}
		case BLEND_SUBTRACT: {
			return c_b - c_s;
		}
		case BLEND_DIVIDE: {
			// A black backdrop stays black, and any other turns white when divided by black
			return select(select(c_b / c_s, 1., c_s <= BLEND_EDGE_TOLERANCE), 0., c_b <= BLEND_EDGE_TOLERANCE);
		}
		case BLEND_NORMAL, default: {
			return c_s;
		}
	}
}

// The two colors mixed by the blend mode's own formula, with the foreground's alpha, matching `apply_blend_mode`
fn apply_blend_mode(foreground: vec4<f32>, background: vec4<f32>, mode: u32) -> vec4<f32> {
	switch mode {
		case BLEND_DARKER_COLOR: {
			let background_is_darker = (background.r + background.g + background.b) / 3. <= (foreground.r + foreground.g + foreground.b) / 3.;
			return vec4<f32>(select(foreground.rgb, background.rgb, background_is_darker), foreground.a);
		}
		case BLEND_LIGHTER_COLOR: {
			let background_is_lighter = (background.r + background.g + background.b) / 3. >= (foreground.r + foreground.g + foreground.b) / 3.;
			return vec4<f32>(select(foreground.rgb, background.rgb, background_is_lighter), foreground.a);
		}
		case BLEND_HUE, BLEND_SATURATION, BLEND_COLOR, BLEND_LUMINOSITY: {
			// The component modes work on gamma-encoded channels, with light brighter than white clipped since they're defined only across 0..1
			let backdrop = clamp(to_gamma(background).rgb, vec3<f32>(0.), vec3<f32>(1.));
			let source = clamp(to_gamma(foreground).rgb, vec3<f32>(0.), vec3<f32>(1.));

			var mixed = vec3<f32>(0.);
			if mode == BLEND_HUE {
				let saturated = set_saturation(source, channel_range(backdrop));
				mixed = set_luminosity(saturated, luma_rec_601_rounded(saturated), luma_rec_601_rounded(backdrop));
			} else if mode == BLEND_SATURATION {
				let saturated = set_saturation(backdrop, channel_range(source));
				mixed = set_luminosity(saturated, luma_rec_601_rounded(saturated), luma_rec_601_rounded(backdrop));
			} else if mode == BLEND_COLOR {
				mixed = set_luminosity(source, luma_rec_601_rounded(source), luma_rec_601_rounded(backdrop));
			} else {
				mixed = set_luminosity(backdrop, luma_rec_601_rounded(backdrop), luma_rec_601_rounded(source));
			}
			return from_gamma(vec4<f32>(mixed, foreground.a));
		}
		case BLEND_NORMAL, BLEND_DARKEN, BLEND_MULTIPLY, BLEND_COLOR_BURN, BLEND_LINEAR_BURN, BLEND_LIGHTEN, BLEND_SCREEN, BLEND_COLOR_DODGE, BLEND_LINEAR_DODGE, BLEND_OVERLAY,
			BLEND_SOFT_LIGHT, BLEND_HARD_LIGHT, BLEND_VIVID_LIGHT, BLEND_LINEAR_LIGHT, BLEND_PIN_LIGHT, BLEND_HARD_MIX, BLEND_DIFFERENCE, BLEND_EXCLUSION, BLEND_SUBTRACT, BLEND_DIVIDE: {
			let red = blend_channel(mode, background.r, foreground.r);
			let green = blend_channel(mode, background.g, foreground.g);
			let blue = blend_channel(mode, background.b, foreground.b);
			return vec4<f32>(clamp(vec3<f32>(red, green, blue), vec3<f32>(0.), vec3<f32>(1.)), foreground.a);
		}
		case BLEND_ERASE, BLEND_RESTORE, BLEND_MULTIPLY_ALPHA, default: {
			// The alpha-only utility modes mix no color, so the foreground passes through
			return foreground;
		}
	}
}

// The blend mode (x) and opacity (y), then the color, mixed over in gamma, matching `overlay_paint`
fn color_overlay(color: vec4<f32>, at: u32) -> vec4<f32> {
	let settings = step_texel(at);
	let paint = step_texel(at + 1u);
	let blended = blend_gamma(color.rgb, to_gamma(paint).rgb, u32(settings.x));

	return vec4<f32>(color.rgb + (blended - color.rgb) * (settings.y * paint.a), color.a);
}

// The blend mode's mix of straight gamma-encoded `backdrop` and `paint` colors, matching `blend_gamma`
fn blend_gamma(backdrop: vec3<f32>, paint: vec3<f32>, mode: u32) -> vec3<f32> {
	switch mode {
		// The component modes encode the colors they're given themselves
		case BLEND_HUE, BLEND_SATURATION, BLEND_COLOR, BLEND_LUMINOSITY: {
			return to_gamma(apply_blend_mode(from_gamma(vec4<f32>(paint, 1.)), from_gamma(vec4<f32>(backdrop, 1.)), mode)).rgb;
		}
		default: {
			return apply_blend_mode(vec4<f32>(paint, 1.), vec4<f32>(backdrop, 1.), mode).rgb;
		}
	}
}
