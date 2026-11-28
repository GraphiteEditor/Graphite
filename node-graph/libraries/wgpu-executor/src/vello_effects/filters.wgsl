
// =======
// Filters
// =======

// The filters work on the same premultiplied gamma-encoded pixels as the passes above, sharing their bindings and parameters

// A texel's straight color with an alpha of 1 if the full-strength content (`secondary`) covers it at least `values.x`, or nothing if it doesn't or lies beyond the edges
fn visible_color_at(pixel: vec2<i32>) -> vec4<f32> {
	let size = vec2<i32>(textureDimensions(source));
	if any(pixel < vec2<i32>(0)) || any(pixel >= size) {
		return vec4<f32>(0.);
	}
	let color = textureLoad(source, pixel, 0);
	let visible = color.a > 0. && textureLoad(secondary, pixel, 0).a >= parameters.values.x;
	return select(vec4<f32>(0.), vec4<f32>(color.rgb / color.a, 1.), visible);
}

// Each texel's straight color as the filters read it, matching the CPU's `visible_colors`: its own if it's visible, otherwise that of the first visible texel `taps[i].xy` texels away,
// or failing that its own if it has any coverage. A pass after others (`values.z` 1) keeps the color they found (`content`), and only the last (`values.w` 1) falls back to the texel's own.
@fragment
fn visible_colors(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	if parameters.values.z > 0.5 {
		let found = textureLoad(content, pixel, 0);
		if found.a > 0.5 {
			return found;
		}
	}

	for (var i = 0u; i < parameters.tap_count; i++) {
		let color = visible_color_at(pixel + vec2<i32>(parameters.taps[i].xy));
		if color.a > 0.5 {
			return color;
		}
	}

	let own = textureLoad(source, pixel, 0);
	if parameters.values.w > 0.5 && own.a > 0. {
		return vec4<f32>(own.rgb / own.a, 1.);
	}
	return vec4<f32>(0.);
}

// The content composited over an opaque gray of level `values.z`, which a matted filter reads
@fragment
fn matte_colors(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let color = textureLoad(source, vec2<i32>(position.xy), 0);
	return vec4<f32>(color.rgb + parameters.values.z * (1. - color.a), 1.);
}

// A texel of the colors a filter reads, with an alpha of 1 where there's one, or beyond the edges nothing, or with `values.y` at or above 0 the opaque gray of that level
fn color_at(pixel: vec2<i32>) -> vec4<f32> {
	let size = vec2<i32>(textureDimensions(source));
	if any(pixel < vec2<i32>(0)) || any(pixel >= size) {
		return select(vec4<f32>(0.), vec4<f32>(vec3<f32>(parameters.values.y), 1.), parameters.values.y >= 0.);
	}
	return textureLoad(source, pixel, 0);
}

// The colors at a position in texels from the first texel's center, interpolated between those of the four texels around it with a color, matching the CPU's `BilinearTap`
fn color_between(position: vec2<f32>) -> vec3<f32> {
	let whole = floor(position);
	let fraction = position - whole;
	let pixel = vec2<i32>(whole);

	let top = mix(color_at(pixel), color_at(pixel + vec2<i32>(1, 0)), fraction.x);
	let bottom = mix(color_at(pixel + vec2<i32>(0, 1)), color_at(pixel + vec2<i32>(1, 1)), fraction.x);
	let between = mix(top, bottom, fraction.y);
	return select(vec3<f32>(0.), between.rgb / between.a, between.a > 0.);
}

// The weighted sum of the colors (`source`) read `taps[i].xy` texels away, each weighted by `taps[i].z`, raised by the offset `values.x` and clamped,
// matching the CPU's `convolve` before the content's coverage returns. A pass after others (`values.z` 1) adds to their sum (`content`), and only the last (`values.w` 1) clamps it.
@fragment
fn convolve_taps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let center = vec2<f32>(pixel);

	var sum = vec3<f32>(parameters.values.x);
	if parameters.values.z > 0.5 {
		sum = textureLoad(content, pixel, 0).rgb;
	}
	for (var i = 0u; i < parameters.tap_count; i++) {
		let tap = parameters.taps[i];
		sum += color_between(center + tap.xy) * tap.z;
	}

	if parameters.values.w > 0.5 {
		return vec4<f32>(clamp(sum, vec3<f32>(0.), vec3<f32>(1.)), 1.);
	}
	return vec4<f32>(sum, 1.);
}

// For PSD interop, each channel of the colors matted over white (`source`) darkened by the length of its Sobel gradient, matching the CPU's `find_edges`.
// Each tap `taps[i].xy` texels away weighs `taps[i].z` across and `taps[i].w` down.
@fragment
fn find_edges(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let center = vec2<f32>(vec2<i32>(position.xy));

	var across = vec3<f32>(0.);
	var down = vec3<f32>(0.);
	for (var i = 0u; i < parameters.tap_count; i++) {
		let tap = parameters.taps[i];
		let color = color_between(center + tap.xy);
		across += color * tap.z;
		down += color * tap.w;
	}
	return vec4<f32>(clamp(1. - sqrt(across * across + down * down), vec3<f32>(0.), vec3<f32>(1.)), 1.);
}

// Filtered opaque colors (`source`) given the `content` texel's coverage, matching the CPU's `uncovered`: as straight colors, or with `values.z` at or above 0, lifted off the gray of that level they were filtered over
@fragment
fn uncover(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let colors = textureLoad(source, pixel, 0).rgb;
	let alpha = textureLoad(content, pixel, 0).a;

	let gray = parameters.values.z;
	if gray < 0. {
		return vec4<f32>(colors * alpha, alpha);
	}
	return vec4<f32>(clamp(colors - gray * (1. - alpha), vec3<f32>(0.), vec3<f32>(alpha)), alpha);
}

// The colors and coverage a neighborhood maximum (with `values.z` 1) or minimum works on, matching the CPU's `neighborhood_extreme`:
// the straight colors of covered texels for the maximum, black elsewhere, or for the minimum the colors matted over white
@fragment
fn neighborhood_prepare(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let color = textureLoad(source, vec2<i32>(position.xy), 0);
	if parameters.values.z > 0.5 {
		return select(vec4<f32>(0.), vec4<f32>(color.rgb / color.a, color.a), color.a > 0.);
	}
	return vec4<f32>(color.rgb + 1. - color.a, color.a);
}

// The colors' extreme of two texels (the maximum with `values.z` 1, otherwise the minimum), and the opposite extreme of their coverages
fn neighborhood_extremes(first: vec4<f32>, second: vec4<f32>) -> vec4<f32> {
	if parameters.values.z > 0.5 {
		return vec4<f32>(max(first.rgb, second.rgb), min(first.a, second.a));
	}
	return vec4<f32>(min(first.rgb, second.rgb), max(first.a, second.a));
}

// A texel of what a neighborhood works on (`source`), or beyond the edges, the colors at their identity and no coverage
fn neighborhood_texel(pixel: vec2<i32>) -> vec4<f32> {
	let size = vec2<i32>(textureDimensions(source));
	if any(pixel < vec2<i32>(0)) || any(pixel >= size) {
		return vec4<f32>(vec3<f32>(select(1., 0., parameters.values.z > 0.5)), 0.);
	}
	return textureLoad(source, pixel, 0);
}

// The extremes over `values.x` texels each way along `step`, read one by one, matching the CPU's `window_extremes`
@fragment
fn neighborhood_extreme(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let step = vec2<i32>(parameters.step);
	let radius = i32(parameters.values.x);

	var extremes = textureLoad(source, pixel, 0);
	for (var offset = 1; offset <= radius; offset++) {
		extremes = neighborhood_extremes(extremes, neighborhood_texel(pixel - step * offset));
		extremes = neighborhood_extremes(extremes, neighborhood_texel(pixel + step * offset));
	}
	return extremes;
}

// The extremes (`source`) widened by `values.x` texels each way along `step`, from the two windows that far to either side, which span the wider one if `values.x` is at most their reach.
// One past an edge reads the window at the edge instead, which lies within the wider one and also reaches beyond the edge.
@fragment
fn neighborhood_widen(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let last = vec2<i32>(textureDimensions(source)) - 1;
	let shift = vec2<i32>(parameters.step) * i32(parameters.values.x);

	let before = textureLoad(source, clamp(pixel - shift, vec2<i32>(0), last), 0);
	let after = textureLoad(source, clamp(pixel + shift, vec2<i32>(0), last), 0);
	return neighborhood_extremes(before, after);
}

// The extremes of a maximum (with `values.z` 1) or minimum back as premultiplied colors, the minimum's lifted back off white within the grown coverage
fn premultiplied_extremes(extremes: vec4<f32>) -> vec4<f32> {
	if parameters.values.z > 0.5 {
		return vec4<f32>(extremes.rgb * extremes.a, extremes.a);
	}
	return vec4<f32>(clamp(extremes.rgb - (1. - extremes.a), vec3<f32>(0.), vec3<f32>(extremes.a)), extremes.a);
}

// The extremes (`source`) back as premultiplied colors, mixed toward those of the next whole radius (`secondary`) by the fraction `values.x`, matching the CPU's `neighborhood_extreme`
@fragment
fn neighborhood_finish(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let near = premultiplied_extremes(textureLoad(source, pixel, 0));
	let far = premultiplied_extremes(textureLoad(secondary, pixel, 0));
	return mix(near, far, parameters.values.x);
}

// The texels (`source`) read `taps[i].xy` texels away between those around them, each weighted by `taps[i].z`, with none beyond the edges (`values.y` below 0), matching the CPU's `neighbors_average`.
// A pass after others (`values.z` 1) adds to their sum (`content`), and the last (`values.w` 1) clamps it.
@fragment
fn sum_taps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);

	var sum = vec4<f32>(0.);
	if parameters.values.z > 0.5 {
		sum = textureLoad(content, pixel, 0);
	}
	for (var i = 0u; i < parameters.tap_count; i++) {
		let tap = parameters.taps[i];
		let position = vec2<f32>(pixel) + tap.xy;
		let whole = vec2<i32>(floor(position));
		let fraction = position - floor(position);

		let top = mix(color_at(whole), color_at(whole + vec2<i32>(1, 0)), fraction.x);
		let bottom = mix(color_at(whole + vec2<i32>(0, 1)), color_at(whole + vec2<i32>(1, 1)), fraction.x);
		sum += mix(top, bottom, fraction.y) * tap.z;
	}

	if parameters.values.w > 0.5 {
		return clamp(sum, vec4<f32>(0.), vec4<f32>(1.));
	}
	return sum;
}

// The texels' straight colors made opaque where visible in the full-strength content in `secondary` and the rest left out, so dividing their blur by its alpha averages the visible colors alone
@fragment
fn visible_only(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let color = textureLoad(source, pixel, 0);
	let visible = color.a > 0. && textureLoad(secondary, pixel, 0).a >= parameters.values.z;
	return select(vec4<f32>(0.), vec4<f32>(color.rgb / color.a, 1.), visible);
}

// The blurred visibilities between which unsharp masking eases from comparing a texel against its own color to the visible colors' average, matching the CPU's
const AVERAGE_VISIBILITY_RAMP: vec2<f32> = vec2<f32>(8. / 255., 24. / 255.);

// The average of the visible colors a texel's own straight color is compared against, from the blur of them made opaque among the rest left out (`blurred`),
// easing into its own color where too little of the blur lands on visible colors to know their average
fn visible_average(own: vec3<f32>, blurred: vec4<f32>) -> vec3<f32> {
	let weight = clamp((blurred.a - AVERAGE_VISIBILITY_RAMP.x) / (AVERAGE_VISIBILITY_RAMP.y - AVERAGE_VISIBILITY_RAMP.x), 0., 1.);
	return select(own, own + (blurred.rgb / blurred.a - own) * weight, weight > 0.);
}

// For PSD interop, the middle gray a high pass centers its differences on, matching the CPU's
const HIGH_PASS_GRAY: f32 = 128. / 255.;

// For PSD interop, the Sobel gradient lengths between which Edges Only ramps from no change to the whole of it, matching the CPU's
const EDGE_GAIN_RAMP: vec2<f32> = vec2<f32>(0.25, 1.);

// The image's coverage (`secondary`) at a position in texels from the first texel's center, interpolated between the four texels around it, with none beyond the edges
fn coverage_between(position: vec2<f32>) -> f32 {
	let whole = floor(position);
	let fraction = position - whole;
	let size = vec2<i32>(textureDimensions(secondary));

	var corners = array<f32, 4>(0., 0., 0., 0.);
	for (var corner = 0; corner < 4; corner++) {
		let at = vec2<i32>(whole) + vec2<i32>(corner % 2, corner / 2);
		if all(at >= vec2<i32>(0)) && all(at < size) {
			corners[corner] = textureLoad(secondary, at, 0).a;
		}
	}
	return mix(mix(corners[0], corners[1], fraction.x), mix(corners[2], corners[3], fraction.x), fraction.y);
}

// For PSD interop, the share of its change Edges Only makes in each channel, by the length of the channel's Sobel gradient, matching the CPU's `edge_gains`.
// Each tap `taps[i].xy` texels away weighs `taps[i].z` across and `taps[i].w` down, read on the continued colors (`source`) and the image's coverage (`secondary`).
@fragment
fn edge_gains(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let center = vec2<f32>(vec2<i32>(position.xy));

	var across = vec4<f32>(0.);
	var down = vec4<f32>(0.);
	for (var i = 0u; i < parameters.tap_count; i++) {
		let tap = parameters.taps[i];
		let value = vec4<f32>(color_between(center + tap.xy), coverage_between(center + tap.xy));
		across += value * tap.z;
		down += value * tap.w;
	}

	let gradient = sqrt(across * across + down * down);
	return clamp((gradient - EDGE_GAIN_RAMP.x) / (EDGE_GAIN_RAMP.y - EDGE_GAIN_RAMP.x), vec4<f32>(0.), vec4<f32>(1.));
}

// What the unsharp mask compares the image (`source`) against for Edges Only: its straight colors moved toward their neighbors' visible average (`secondary`) by the share their edge gains (`content`) give,
// made opaque so the unsharp mask reads them as the whole average. With `values.z` 1, it's the coverage moved toward its neighbors' average instead, matching the CPU's `unsharp_mask`.
@fragment
fn edges_only_average(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let image = textureLoad(source, pixel, 0);
	let neighbors = textureLoad(secondary, pixel, 0);
	let gains = textureLoad(content, pixel, 0);

	if parameters.values.z > 0.5 {
		return vec4<f32>(0., 0., 0., image.a + (neighbors.a - image.a) * gains.a);
	}
	if image.a <= 0. {
		return vec4<f32>(0.);
	}
	let own = image.rgb / image.a;
	return vec4<f32>(own + (visible_average(own, neighbors) - own) * gains.rgb, 1.);
}

// For PSD interop, unsharp masking's change to values lying `difference` above the blur around them, by the amount `values.x` and threshold `values.y`, matching the CPU's `unsharp_change`
fn unsharp_change(difference: vec4<f32>) -> vec4<f32> {
	return sign(difference) * max(parameters.values.x * abs(difference) - parameters.values.y, vec4<f32>(0.));
}

// Sharpens the straight colors and coverage of the image (`source`) against the blur of its visible colors (`secondary`) and of its coverage (`content`), matching the CPU's `unsharp_mask`.
// With `values.w` 1, it's a high pass instead: the colors' difference from their blur around middle gray, with the coverage kept.
@fragment
fn unsharp_mask(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let image = textureLoad(source, pixel, 0);
	if image.a <= 0. {
		return vec4<f32>(0.);
	}
	let own = vec4<f32>(image.rgb / image.a, image.a);

	let average = visible_average(own.rgb, textureLoad(secondary, pixel, 0));
	let difference = own - vec4<f32>(average, textureLoad(content, pixel, 0).a);

	var sharpened = clamp(own + unsharp_change(difference), vec4<f32>(0.), vec4<f32>(1.));
	if parameters.values.w > 0.5 {
		sharpened = vec4<f32>(clamp(difference.rgb + HIGH_PASS_GRAY, vec3<f32>(0.), vec3<f32>(1.)), image.a);
	}
	return vec4<f32>(sharpened.rgb * sharpened.a, sharpened.a);
}

// Each texel's share of the full-strength content's coverage (`content`) that the faded fill (`source`) left, matching the CPU's `fill_shares`
@fragment
fn fill_shares(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let filled = textureLoad(source, pixel, 0).a;
	let full = textureLoad(content, pixel, 0).a;
	return vec4<f32>(0., 0., 0., select(0., min(filled / full, 1.), full > 0.));
}

// The fill's filtered straight colors (`source`) at the coverage of the full-strength content's filtered result (`secondary`) scaled by each texel's share of it (`content`),
// matching the CPU's `faded_by_shares`
@fragment
fn faded_by_shares(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let filled = textureLoad(source, pixel, 0);
	let full = textureLoad(secondary, pixel, 0);
	let alpha = full.a * textureLoad(content, pixel, 0).a;

	var colors = vec3<f32>(0.);
	if filled.a > 0. {
		colors = filled.rgb / filled.a;
	} else if full.a > 0. {
		colors = full.rgb / full.a;
	}
	return vec4<f32>(colors * alpha, alpha);
}

// =============
// Random fields
// =============

// For PSD interop, how many standard deviations Gaussian noise reaches, matching the CPU's
const GAUSSIAN_NOISE_LIMIT: f32 = 3.5;

// PCG3D from Jarzynski and Olano's "Hash Functions for GPU Rendering", matching the CPU's
fn pcg3d(input: vec3<u32>) -> vec3<u32> {
	var v = input * 1664525u + 1013904223u;
	v.x += v.y * v.z;
	v.y += v.z * v.x;
	v.z += v.x * v.y;
	v ^= v >> vec3<u32>(16u);
	v.x += v.y * v.z;
	v.y += v.z * v.x;
	v.z += v.x * v.y;
	return v;
}

// Values strictly between 0 and 1 from the top 23 bits of each hash, which an f32 holds exactly with the half added, matching the CPU's `unit_interval`
fn unit_interval(hash: vec3<u32>) -> vec3<f32> {
	return (vec3<f32>(hash >> vec3<u32>(9u)) + 0.5) / 8388608.;
}

// A 32-bit integer from its exact low (x) and high (y) 16-bit halves
fn joined_halves(halves: vec2<f32>) -> u32 {
	return u32(halves.x) | (u32(halves.y) << 16u);
}

// The first or `second` field's seed, from its exact halves in `taps[3]`
fn field_seed(second: bool) -> u32 {
	return joined_halves(select(parameters.taps[3].xy, parameters.taps[3].zw, second));
}

// The three values a field of the `seed` holds at the cell `offset` texels from the texel's middle at `position`, matching the CPU's `FieldSampling`.
// `taps[0]` holds the texels' axes in cells, `taps[1].xy` the rest of the way to the origin, and `taps[4]` the origin's cell in exact halves.
fn field_values(position: vec2<f32>, offset: vec2<f32>, seed: u32) -> vec3<f32> {
	let axes = mat2x2<f32>(parameters.taps[0].xy, parameters.taps[0].zw);
	let origin_cell = vec2<u32>(joined_halves(parameters.taps[4].xy), joined_halves(parameters.taps[4].zw));
	let cell = origin_cell + bitcast<vec2<u32>>(vec2<i32>(floor(parameters.taps[1].xy + axes * (position + offset))));
	return unit_interval(pcg3d(vec3<u32>(cell, seed)));
}

// How many samples a texel takes along each axis, from `taps[2].xy`
fn sample_counts() -> vec2<u32> {
	return vec2<u32>(parameters.taps[2].xy);
}

// The offset in texels of a texel's sample in `column` and `row`, spread as far as `taps[2].zw` from the first to the last
fn sample_offset(column: u32, row: u32) -> vec2<f32> {
	return ((vec2<f32>(f32(column), f32(row)) + 0.5) / parameters.taps[2].xy - 0.5) * parameters.taps[2].zw;
}

// The offsets Add Noise gives a texel's colors, matching the CPU's `add_noise`: its noise averaged over its samples (Gaussian with `values.z` 1), the same for every color with `values.w` 1,
// scaled by `values.x` and kept within `values.y` either way
fn noise_offsets(position: vec2<f32>) -> vec3<f32> {
	let counts = sample_counts();
	var sum = vec3<f32>(0.);
	for (var row = 0u; row < counts.y; row++) {
		for (var column = 0u; column < counts.x; column++) {
			let offset = sample_offset(column, row);
			let offsets = field_values(position, offset, field_seed(false));

			// The Box-Muller transform of two uniform values into one with a normal distribution
			if parameters.values.z > 0.5 {
				let angles = field_values(position, offset, field_seed(true));
				sum += clamp(sqrt(-2. * log(offsets)) * cos(6.2831855 * angles), vec3<f32>(-GAUSSIAN_NOISE_LIMIT), vec3<f32>(GAUSSIAN_NOISE_LIMIT));
			} else {
				sum += 2. * offsets - 1.;
			}
		}
	}

	var noise = sum / f32(counts.x * counts.y);
	if parameters.values.w > 0.5 {
		noise = vec3<f32>(noise.x);
	}
	return clamp(noise * parameters.values.x, vec3<f32>(-parameters.values.y), vec3<f32>(parameters.values.y));
}

// The offsets Add Noise gives each color, around middle gray and scaled to fill the range, which an SVG adds to the colors
@fragment
fn noise_map(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return vec4<f32>(0.5 + noise_offsets(position.xy) / (2. * parameters.values.y), 1.);
}

// Each straight color offset by noise and clamped, with the coverage kept
@fragment
fn add_noise(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let color = textureLoad(source, vec2<i32>(position.xy), 0);
	if color.a <= 0. {
		return vec4<f32>(0.);
	}
	let noisy = clamp(color.rgb / color.a + noise_offsets(position.xy), vec3<f32>(0.), vec3<f32>(1.));
	return vec4<f32>(noisy * color.a, color.a);
}

// How much of a texel Dissolve shows, matching the CPU's `dissolve_field`: the share of its samples passing the opacity `values.x`, deviating from it less by `values.y`,
// and the threshold level from 0 to 254 its coverage must pass
fn dissolve_field(position: vec2<f32>) -> vec2<f32> {
	let counts = sample_counts();
	let seed = field_seed(false);
	var shown = 0.;
	for (var row = 0u; row < counts.y; row++) {
		for (var column = 0u; column < counts.x; column++) {
			shown += select(0., 1., field_values(position, sample_offset(column, row), seed).x < parameters.values.x);
		}
	}

	let opacity = parameters.values.x;
	let share = clamp(opacity + (shown / f32(counts.x * counts.y) - opacity) * parameters.values.y, 0., 1.);
	let threshold = floor(field_values(position, vec2<f32>(0.), seed).y * 255.);
	return vec2<f32>(share, threshold);
}

// The threshold level as a gray, a fraction of 255, at the share shown as its alpha, which an SVG combines with the coverage. Gray survives a conversion between color spaces.
@fragment
fn dissolve_map(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let field = dissolve_field(position.xy);
	return vec4<f32>(vec3<f32>(field.y / 255. * field.x), field.x);
}

// The content shown fully opaque by the share where its coverage is at least one 8-bit level above the threshold, and hidden elsewhere
@fragment
fn dissolve(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let color = textureLoad(source, vec2<i32>(position.xy), 0);
	if color.a <= 0. {
		return vec4<f32>(0.);
	}
	let field = dissolve_field(position.xy);
	let shown = select(0., field.x, color.a * 255. > field.y + 0.5);
	return vec4<f32>(color.rgb / color.a * shown, shown);
}
