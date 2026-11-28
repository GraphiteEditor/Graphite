// The passes that mask, feather, and blur content by a radius mask, appended to `vello_effects.wgsl` and sharing its bindings and parameters

// SVG's `luminanceToAlpha` weights, taken on gamma-encoded channels, matching `MASK_LUMINANCE_WEIGHTS`
const MASK_LUMINANCE_WEIGHTS: vec3<f32> = vec3<f32>(0.2125, 0.7154, 0.0721);

// How much the mask graphic in `source`, premultiplied, shows the content: its luminance (times its alpha) with `values.x` 0, or its alpha with 1
@fragment
fn mask_coverage(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let mask = textureLoad(source, vec2<i32>(position.xy), 0);
	let coverage = select(dot(mask.rgb, MASK_LUMINANCE_WEIGHTS), mask.a, parameters.values.x > 0.5);
	return vec4<f32>(clamp(coverage, 0., 1.), 0., 0., 0.);
}

// The mask's feathered coverage in `source` mapped through its density line, of slope `values.x` and intercept `values.y`
@fragment
fn mask_density_line(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let coverage = textureLoad(source, vec2<i32>(position.xy), 0).r;
	return vec4<f32>(clamp(parameters.values.x * coverage + parameters.values.y, 0., 1.));
}

// The image in `source` faded by the strength from 0 to 1 in `secondary`
@fragment
fn apply_mask(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	return textureLoad(source, pixel, 0) * textureLoad(secondary, pixel, 0).r;
}

// The alpha of the full-strength content in `source` with the trailing masks' strength (`secondary`) in red, standing in for that content where `layer_opacity` reads both
@fragment
fn content_with_layer_fade(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	return vec4<f32>(textureLoad(secondary, pixel, 0).r, 0., 0., textureLoad(source, pixel, 0).a);
}

// The alpha at or below which a feather counts content as absent, matching `FEATHER_FOOTPRINT_ALPHA`
const FEATHER_FOOTPRINT_ALPHA: f32 = 0.5 / 255.;

// The coverage of the area outside the footprint of the content in `source`, with its edge falling within the texels bordering it by their alpha relative to the largest beside them
@fragment
fn feather_outside(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let size = vec2<i32>(textureDimensions(source));
	let alpha = textureLoad(source, pixel, 0).a;
	if alpha <= FEATHER_FOOTPRINT_ALPHA {
		return vec4<f32>(1.);
	}

	var borders_absence = false;
	var largest = alpha;
	for (var neighbor_y = -1; neighbor_y <= 1; neighbor_y++) {
		for (var neighbor_x = -1; neighbor_x <= 1; neighbor_x++) {
			let neighbor = pixel + vec2<i32>(neighbor_x, neighbor_y);
			if any(neighbor < vec2<i32>(0)) || any(neighbor >= size) {
				continue;
			}
			let neighbor_alpha = textureLoad(source, neighbor, 0).a;
			borders_absence = borders_absence || neighbor_alpha <= FEATHER_FOOTPRINT_ALPHA;
			largest = max(largest, neighbor_alpha);
		}
	}
	return vec4<f32>(select(0., 1. - min(alpha / largest, 1.), borders_absence));
}

// How much the footprint eroded by `values.x` texels covers each texel, given its distance to the area outside the footprint plus half a texel in `source`,
// relative to its own coverage of the footprint (one less the outside coverage in `secondary`), matching the CPU's `erode`
@fragment
fn feather_erode(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let coverage = 1. - textureLoad(secondary, pixel, 0).r;
	let eroded = clamp(textureLoad(source, pixel, 0).r - parameters.values.x, 0., 1.);
	return vec4<f32>(select(eroded, min(eroded / coverage, 1.), coverage > 0.));
}

// The luminance of a masked radius blur's map in `source`, premultiplied, rounded to its 8-bit code to match `masked_radius_blur_code`, or with `values.x` 1 unrounded for a box blur
@fragment
fn masked_radius_blur_luminance(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let map = textureLoad(source, vec2<i32>(position.xy), 0);
	let luminance = clamp(dot(map.rgb, MASK_LUMINANCE_WEIGHTS), 0., 1.);
	if parameters.values.x > 0.5 {
		return vec4<f32>(luminance);
	}
	return vec4<f32>(round(luminance * 255.) / 255.);
}

// The level below the map luminance in `source` and the share of the level above, looked up in the `secondary` row by the luminance's 8-bit code, matching `masked_radius_blur_interpolation`
@fragment
fn masked_radius_blur_interpolation(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let code = i32(round(textureLoad(source, vec2<i32>(position.xy), 0).r * 255.));
	return textureLoad(secondary, vec2<i32>(code, 0), 0);
}

// Adds the masked radius blur's level `values.x` in `source`, weighed by its share of each texel's interpolation in `secondary`, to the sum so far in `content` (with `values.y` 1)
@fragment
fn accumulate_masked_radius_blur_level(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let interpolation = textureLoad(secondary, pixel, 0);
	let level = parameters.values.x;

	var weight = 0.;
	if abs(interpolation.r - level) < 0.5 {
		weight = 1. - interpolation.g;
	} else if abs(interpolation.r + 1. - level) < 0.5 {
		weight = interpolation.g;
	}

	var sum = vec4<f32>(0.);
	if parameters.values.y > 0.5 {
		sum = textureLoad(content, pixel, 0);
	}
	return sum + textureLoad(source, pixel, 0) * weight;
}

// A masked box blur's running sums (a summed-area table) count each channel in whole steps, as a low digit below this and a high digit counting how many of it the sum holds.
// f32 holds whole numbers below 2^24 exactly, so adding 16 low digits, or 4 high digits below 2^22, stays exact at any image size.
const RUNNING_SUM_DIGIT: f32 = 1048576.;

// The counts of two channels of `source` (red and green with `values.x` 0, blue and alpha with 1) in `values.y` steps from 0 to 1, up to 16 as `VARIABLE_BOX_MAX_VALUE` gives, as low digits
@fragment
fn variable_box_digits(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let color = textureLoad(source, vec2<i32>(position.xy), 0);
	let channels = select(color.rg, color.ba, parameters.values.x > 0.5);
	let counts = round(clamp(channels, vec2<f32>(0.), vec2<f32>(16.)) * parameters.values.y);
	return vec4<f32>(counts.x, 0., counts.y, 0.);
}

// Adds to each texel's running sums in `content` those ending each block of `values.z` texels before it along the axis `values.xy`, within a block 16 times as long,
// so passes of blocks 1, 16, 256, and so on long sum whole rows or columns
@fragment
fn variable_box_running_sums(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let axis = vec2<i32>(parameters.values.xy);
	let block = i32(parameters.values.z);
	let within = dot(pixel, axis) % (16 * block);

	var sums = textureLoad(content, pixel, 0);
	for (var before = 0; before < within / block; before++) {
		sums += textureLoad(content, pixel + axis * ((before + 1) * block - 1 - within), 0);
	}

	// Each low digit's overflow carries into its high digit
	let carries = floor(sums.xz / RUNNING_SUM_DIGIT);
	return vec4<f32>(sums.x - carries.x * RUNNING_SUM_DIGIT, sums.y + carries.x, sums.z - carries.y * RUNNING_SUM_DIGIT, sums.w + carries.y);
}

// The running sums over the texels above and left of a corner between texels, none along the top and left edges
fn running_sums_at(corner: vec2<i32>) -> vec4<f32> {
	if any(corner <= vec2<i32>(0)) {
		return vec4<f32>(0.);
	}
	return textureLoad(content, corner - vec2<i32>(1), 0);
}

// The two channels' counts over the texels from `start` up to `end`, within the image, exact until each one's digits join
fn counts_within(start: vec2<i32>, end: vec2<i32>) -> vec2<f32> {
	let outer = running_sums_at(end) + running_sums_at(start);
	let digits = outer - (running_sums_at(vec2<i32>(start.x, end.y)) + running_sums_at(vec2<i32>(end.x, start.y)));
	return digits.yw * RUNNING_SUM_DIGIT + digits.xz;
}

// Along one axis of `size` texels, the start and end of the span that `part` 0 (before the image), 1 (within it), or 2 (after it) takes of the texels from `start` up to `end`,
// and how many times it counts them, repeating the first texel before the image and the last after it
fn continued_span(part: i32, start: i32, end: i32, size: i32) -> vec3<i32> {
	let inner_start = clamp(start, 0, size);
	let inner_end = clamp(end, 0, size);
	if part == 0 {
		return vec3<i32>(0, 1, inner_start - start);
	}
	if part == 2 {
		return vec3<i32>(size - 1, size, end - inner_end);
	}
	return vec3<i32>(inner_start, inner_end, 1);
}

// The two channels' counts over the texels from `start` up to `end`, with those past the edges repeating the nearest edge texel, matching the CPU's `counts_continued`
fn counts_continued(start: vec2<i32>, end: vec2<i32>) -> vec2<f32> {
	let size = vec2<i32>(textureDimensions(content));

	var counts = vec2<f32>(0.);
	for (var row = 0; row < 3; row++) {
		for (var column = 0; column < 3; column++) {
			let columns = continued_span(column, start.x, end.x, size.x);
			let rows = continued_span(row, start.y, end.y, size.y);
			if columns.z <= 0 || rows.z <= 0 {
				continue;
			}
			counts += f32(columns.z) * f32(rows.z) * counts_within(vec2<i32>(columns.x, rows.x), vec2<i32>(columns.y, rows.y));
		}
	}
	return counts;
}

// Box blurs two channels of each texel by its own half width, the fraction of the radius `values.xy` in texels its luminance in `secondary` gives, matching the CPU's `variable_box_blur`.
// The first pass (`values.w` 0) blurs red and green, and the second blue and alpha, taking red and green from the first in `source`, encoding to gamma with `amount` 1.
// The running sums in `content` count in `values.z` steps, and texels of no luminance keep their color from `source`.
@fragment
fn variable_box_blur(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let luminance = textureLoad(secondary, pixel, 0).r;
	if luminance <= 0. {
		return textureLoad(source, pixel, 0);
	}

	// A fractional half width mixes the boxes of the whole half widths either side by that fraction along each axis
	let half_width = luminance * parameters.values.xy;
	let whole = floor(half_width);
	let fraction = half_width - whole;
	var counts = vec2<f32>(0.);
	for (var corner = 0; corner < 4; corner++) {
		let wider = vec2<i32>(corner & 1, corner >> 1u);
		let weights = mix(1. - fraction, fraction, vec2<f32>(wider));
		if weights.x * weights.y <= 0. {
			continue;
		}

		let reach = vec2<i32>(whole) + wider;
		counts += weights.x * weights.y * counts_continued(pixel - reach, pixel + reach + vec2<i32>(1));
	}
	let average = counts / ((2. * half_width.x + 1.) * (2. * half_width.y + 1.) * parameters.values.z);

	if parameters.values.w < 0.5 {
		return vec4<f32>(average, 0., 0.);
	}
	let color = vec4<f32>(textureLoad(source, pixel, 0).rg, average);
	if parameters.amount < 0.5 {
		return color;
	}
	if color.a <= 0. {
		return vec4<f32>(0.);
	}
	return vec4<f32>(linear_to_srgb(max(color.rgb / color.a, vec3<f32>(0.))) * color.a, color.a);
}
