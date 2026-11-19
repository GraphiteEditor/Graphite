// The passes that carry out an item's effects on the pixels of its content, which are premultiplied with gamma-encoded color like the pixels Vello GPU blends

struct Parameters {
	// Converts a target pixel's position into the normalized coordinates the inputs are sampled at
	target_to_uv: vec2<f32>,
	// One step along the axis a kernel runs, in normalized coordinates
	step: vec2<f32>,
	// How far a silhouette is cast, in normalized coordinates
	offset: vec2<f32>,
	// A pass's scalar setting, like a scale factor, fill, opacity, or knockout cover, described where it reads it
	amount: f32,
	tap_count: u32,
	// The premultiplied color a silhouette is tinted
	color: vec4<f32>,
	// Each pass's own settings, described where it reads them
	values: vec4<f32>,
	// Each kernel tap's distance in steps (x) and weight (y)
	taps: array<vec4<f32>, 25>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var secondary: texture_2d<f32>;
@group(0) @binding(2) var content: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var<uniform> parameters: Parameters;

// Distances past the farthest a pass looks, which stand for none found
const FAR: f32 = 65504.;

// One triangle that covers the whole target
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
	let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
	return vec4<f32>(corner * 2. - 1., 0., 1.);
}

fn sample_source(uv: vec2<f32>) -> vec4<f32> {
	return textureSampleLevel(source, linear_sampler, uv, 0.);
}

// A separable kernel along one axis, whose taps sit between texels to weight two of them at once
@fragment
fn convolve(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let uv = position.xy * parameters.target_to_uv;

	var sum = vec4<f32>(0.);
	for (var i = 0u; i < parameters.tap_count; i++) {
		let tap = parameters.taps[i];
		sum += sample_source(uv + parameters.step * tap.x) * tap.y;
	}
	return sum;
}

// Halving a texture samples the corner shared by each block of four texels, averaging them, and doubling it interpolates between texels
@fragment
fn resample(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return sample_source(position.xy * parameters.target_to_uv);
}

// The coverage of the content's silhouette, shifted by the offset, less `amount`
@fragment
fn cast_silhouette(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return vec4<f32>(sample_source(position.xy * parameters.target_to_uv - parameters.offset).a - parameters.amount);
}

// The coverage of the area outside the content's silhouette, shifted by the offset
@fragment
fn cast_inverse_silhouette(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return vec4<f32>(1. - sample_source(position.xy * parameters.target_to_uv - parameters.offset).a);
}

// The length of an offset along the shortest path of steps one texel straight, one diagonally, or a knight's move away, matching `chamfer_length`
fn chamfer_length(offset: vec2<f32>) -> f32 {
	let long = max(abs(offset.x), abs(offset.y));
	let short = min(abs(offset.x), abs(offset.y));
	if 2. * short <= long {
		return (long - 2. * short) + sqrt(5.) * short;
	}
	return sqrt(5.) * (long - short) + sqrt(2.) * (2. * short - long);
}

// Jump flooding keeps each texel's offset from the nearest seed found so far (xy) and whether one is found (z). The `content` input holds the seeds' values.
// With `values.y` 0, the seeds are the covered texels and a distance runs to the edge within the seed, placed by its coverage.
// With `values.y` 1, the seeds are the texels past a grow of `values.z` texels and a distance runs to that grown edge, matching `distances_through_seeds`.
// With `values.y` 2, every texel seeds with its value as a head start, matching `distances_through_seeds` of values that are all finite.
// With `amount` 1, offsets are measured in straight lines rather than by `chamfer_length`.
fn is_seed(value: f32) -> bool {
	if parameters.values.y > 1.5 {
		return true;
	}
	if parameters.values.y > 0.5 {
		// Only those up to 3 texels past it, a band any path in from farther crosses
		return value > parameters.values.z && value <= parameters.values.z + 3.;
	}
	return value > 0.;
}

fn seed_distance(pixel: vec2<i32>, offset: vec2<f32>) -> f32 {
	let value = textureLoad(content, pixel - vec2<i32>(offset), 0).r;
	// Only a feather measures round, so the others skip the square root
	var offset_length: f32;
	if parameters.amount > 0.5 {
		offset_length = length(offset);
	} else {
		offset_length = chamfer_length(offset);
	}
	if parameters.values.y > 1.5 {
		return offset_length + value;
	}
	if parameters.values.y > 0.5 {
		return offset_length + parameters.values.z - value;
	}

	return offset_length + (0.5 - value) + 0.5;
}

@fragment
fn jump_flood_seed(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let seed = is_seed(textureLoad(content, vec2<i32>(position.xy), 0).r);
	return vec4<f32>(0., 0., select(0., 1., seed), 0.);
}

// Takes the nearest of the seeds that the texel and its eight neighbors `values.x` texels away have found so far,
// or with `values.w` 1 its neighbors one chamfer step away (a texel straight or diagonally, or a knight's move), settling jump flooding's near misses
@fragment
fn jump_flood_step(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let size = vec2<i32>(textureDimensions(source));
	let step = i32(parameters.values.x);
	let radius = select(1, 2, parameters.values.w > 0.5);

	var best = textureLoad(source, pixel, 0);
	var best_distance = FAR;
	if best.z > 0.5 {
		best_distance = seed_distance(pixel, best.xy);
	}

	// By rows from the top, the order the CPU visits them in too
	for (var neighbor_y = -radius; neighbor_y <= radius; neighbor_y++) {
		for (var neighbor_x = -radius; neighbor_x <= radius; neighbor_x++) {
			let long = max(abs(neighbor_x), abs(neighbor_y));
			let short = min(abs(neighbor_x), abs(neighbor_y));
			if long == 0 || (long == 2 && short != 1) {
				continue;
			}

			let direction = vec2<i32>(neighbor_x, neighbor_y) * step;
			let neighbor = pixel + direction;
			if any(neighbor < vec2<i32>(0)) || any(neighbor >= size) {
				continue;
			}
			let found = textureLoad(source, neighbor, 0);
			if found.z < 0.5 {
				continue;
			}

			// Offsets run from the seed to the texel
			let candidate = found.xy - vec2<f32>(direction);
			let candidate_distance = seed_distance(pixel, candidate);
			if candidate_distance < best_distance {
				best = vec4<f32>(candidate, 1., 0.);
				best_distance = candidate_distance;
			}
		}
	}
	return best;
}

// Each texel's distance to the nearest seed's edge, or `FAR` if none is found
@fragment
fn jump_flood_distance(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let nearest = textureLoad(source, pixel, 0);
	if nearest.z < 0.5 {
		return vec4<f32>(FAR);
	}
	return vec4<f32>(max(seed_distance(pixel, nearest.xy), 0.));
}

// The silhouette grown by `values.x` texels, given each texel's distance to it, with the grown edge fading out over one texel
@fragment
fn grow(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let to_silhouette = textureLoad(source, vec2<i32>(position.xy), 0).r;
	return vec4<f32>(clamp(parameters.values.x + 1. - to_silhouette, 0., 1.));
}

// A precise glow's strength, falling through half at the edge of a grow of `values.x` texels to nothing a unit past the size of `values.y` units, at `values.z` texels per unit.
// Within the grown shape, given `secondary` with `values.w` 1, it measures how far in from the grown edge each texel lies, matching `precise_falloff`.
@fragment
fn precise_falloff(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let grow = parameters.values.x;
	var measured = textureLoad(source, pixel, 0).r;
	if parameters.values.w > 0.5 && measured <= grow {
		measured = grow - textureLoad(secondary, pixel, 0).r;
	}

	let scale = parameters.values.z;
	let size = parameters.values.y;
	let grow_units = grow / scale;
	return vec4<f32>(clamp(0.5 - 0.5 * (measured / scale - grow_units) / (size - grow_units + 1.), 0., 1.));
}

// A shadow or glow's strength, strengthened to reach full at the fraction `values.x` of it, then reshaped by the contour `secondary` holds in a row with `values.y` 1, matching `shape_strength`
@fragment
fn shape_strength(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let strength = clamp(textureLoad(source, vec2<i32>(position.xy), 0).r / parameters.values.x, 0., 1.);
	if parameters.values.y < 0.5 {
		return vec4<f32>(strength);
	}

	// The contour's samples span its ends, each at a texel's center
	let samples = f32(textureDimensions(secondary).x);
	let uv = vec2<f32>((strength * (samples - 1.) + 0.5) / samples, 0.5);
	return vec4<f32>(clamp(textureSampleLevel(secondary, linear_sampler, uv, 0.).r, 0., 1.));
}

// A glow's premultiplied color from its strength after range and contour (`source`), as the gradient `secondary` holds in a row of straight colors from its start to its end.
// Its alpha is the gradient's, faded with `values.x` 1 as the `content` strength before range and contour falls below `values.y`, then by the opacity `values.z`, matching `GlowFill::color_at`.
@fragment
fn gradient_glow_tint(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let strength = textureLoad(source, pixel, 0).r;

	let samples = f32(textureDimensions(secondary).x);
	let uv = vec2<f32>(((1. - strength) * (samples - 1.) + 0.5) / samples, 0.5);
	let color = textureSampleLevel(secondary, linear_sampler, uv, 0.);

	var alpha = color.a;
	if parameters.values.x > 0.5 {
		alpha = min(alpha, textureLoad(content, pixel, 0).r / parameters.values.y);
	}
	alpha *= parameters.values.z;
	return vec4<f32>(color.rgb * alpha, alpha);
}


// The coverage of where a silhouette doesn't reach, which turns an inner glow from the edges into one from the center
@fragment
fn invert_coverage(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return vec4<f32>(1. - textureLoad(source, vec2<i32>(position.xy), 0).r);
}

// A soft chisel bevel's slope-limited matte: the coverage away from half scaled by `amount` (positive inward, negative outward), each texel's head start across the edge
@fragment
fn slope_limited_matte(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return vec4<f32>(max((textureLoad(source, vec2<i32>(position.xy), 0).r - 0.5) * parameters.amount, 0.));
}

// Satin's shading from two casts of the silhouette: how much one covers without the other, or how much they agree with `amount` 1
@fragment
fn satin(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let difference = abs(textureLoad(source, pixel, 0).r - textureLoad(secondary, pixel, 0).r);
	return vec4<f32>(select(difference, 1. - difference, parameters.amount > 0.5));
}
// A silhouette's coverage in its color
@fragment
fn tint(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return parameters.color * textureLoad(source, vec2<i32>(position.xy), 0).r;
}

// Hides the coverage of what lies beneath behind the `content` at full strength, where the fill `amount` fades the content showing over it.
// The faded content shows what's beneath through 1 - fill × alpha of it, of which only 1 - alpha may show this.
@fragment
fn knock_out(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let alpha = textureLoad(content, pixel, 0).a;
	let shown = (1. - alpha) / max(1. - parameters.amount * alpha, 1e-6);
	return textureLoad(source, pixel, 0) * shown;
}

// Stacks a tinted silhouette with its `secondary` coverage over the shadows and glows beneath the content (`source`, with `values.x` 1).
// With `values.y` 1, `secondary` is already tinted.
@fragment
fn accumulate(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	var tinted = parameters.color * textureLoad(secondary, pixel, 0).r;
	if parameters.values.y > 0.5 {
		tinted = textureLoad(secondary, pixel, 0);
	}

	var beneath = vec4<f32>(0.);
	if parameters.values.x > 0.5 {
		beneath = textureLoad(source, pixel, 0);
	}
	return tinted + beneath * (1. - tinted.a);
}

// Places a tinted part of an interior piece within the full-strength `content`'s share of each pixel of the image (`source`), matching the CPU's `composite_in_share`.
// It mixes with the image's own colors by the blend mode numbered `values.x` in gamma, and with `values.y` 1, `secondary` is already tinted.
// `values.z` picks the part: 0 the whole piece, 1 the part blending with the content, or 2 that part for a piece whose part over the content takes what its fill lets show through, at opacity `amount`.
@fragment
fn composite_within(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let image = textureLoad(source, pixel, 0);
	var tinted = parameters.color * textureLoad(secondary, pixel, 0).r;
	if parameters.values.y > 0.5 {
		tinted = textureLoad(secondary, pixel, 0);
	}
	let share = textureLoad(content, pixel, 0).a;
	if tinted.a <= 0. {
		return image;
	}

	// The paint blended with the image's own straight color, weighted by how much of the share the image covers
	let color = tinted.rgb / tinted.a;
	var blended = vec3<f32>(0.);
	if image.a > 0. {
		blended = blend_gamma(image.rgb / image.a, color, u32(parameters.values.x)) * image.a;
	}

	var strength = tinted.a;
	var plain = 0.;
	let part = u32(parameters.values.z);
	if part == 0u {
		plain = share - image.a;
	} else if part == 2u {
		let opacity = layer_opacity(pixel);
		strength *= 1. - opacity * through_share(share, image.a, opacity);
	}
	return vec4<f32>((1. - strength) * image.rgb + strength * (plain * color + blended), image.a + strength * plain);
}

// The opacity or cover in `amount` as the trailing masks leave it at a pixel, with `values.w` 1 and their strength in the red of `content` (see `content_with_layer_fade`)
fn layer_opacity(pixel: vec2<i32>) -> f32 {
	return parameters.amount * select(1., textureLoad(content, pixel, 0).r, parameters.values.w > 0.5);
}

// The share of a pixel within the content of full-strength alpha `share` that what lies beneath shows through, given the content's layer's alpha, drawn at `opacity`
fn through_share(share: f32, layer_alpha: f32, opacity: f32) -> f32 {
	let shown = 1. - opacity * layer_alpha;
	return select(0., clamp((share - layer_alpha) / shown, 0., 1.), shown > 1e-6);
}

// A tinted part of an interior piece (`source`) cut to the share of each pixel within the full-strength `content` that what lies beneath shows through,
// given the content's finished layer (`secondary`) drawn at opacity `amount`. With `values.x` 1, it's the share alone, in alpha.
@fragment
fn through_mask(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let share = through_share(textureLoad(content, pixel, 0).a, textureLoad(secondary, pixel, 0).a, layer_opacity(pixel));
	if parameters.values.x > 0.5 {
		return vec4<f32>(0., 0., 0., share);
	}
	return textureLoad(source, pixel, 0) * share;
}

// Hides what lies beneath (`source`) behind the full-strength `content` where the content's finished layer (`secondary`) covers it, at `amount` times its alpha.
// That layer shows what's beneath through 1 - amount × its alpha, of which only 1 - the content's alpha may show this.
@fragment
fn knock_out_by_layer(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let shown = (1. - textureLoad(content, pixel, 0).a) / max(1. - layer_opacity(pixel) * textureLoad(secondary, pixel, 0).a, 1e-6);
	return textureLoad(source, pixel, 0) * min(shown, 1.);
}

// A chiseled bevel's height from -0.5 at its foot `values.w` texels outside the edge to 0.5 at its top as far inside, from the distances in from the edge (`source`) and out from it (`secondary`).
// It's kept from `values.x` to `values.y` texels across the edge, and with `values.z` 1 (a soft chisel, given distances through its slope-limited matte) measured by their difference alone. Matches the CPU's `bevel_height`.
@fragment
fn chisel_height(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let inward = textureLoad(source, pixel, 0).r;
	let outward = textureLoad(secondary, pixel, 0).r;

	var rise = max(inward - 0.5, 0.) - max(outward - 0.5, 0.) + 0.5;
	if parameters.values.z > 0.5 {
		rise = inward - outward;
	}
	return vec4<f32>(clamp(rise, parameters.values.x, parameters.values.y) / (2. * parameters.values.w));
}

// Places the shadows and glows beneath the image
@fragment
fn composite_beneath(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let image = textureLoad(source, pixel, 0);
	let beneath = textureLoad(secondary, pixel, 0);

	return image + beneath * (1. - image.a);
}

// The paint mixed with the backdrop by a blend mode besides the component ones, numbered in the order `BlendMode` declares them
fn blend(backdrop: vec3<f32>, paint: vec3<f32>, mode: u32) -> vec3<f32> {
	let average = vec3<f32>(1. / 3.);
	switch mode {
		// Darker Color and Lighter Color keep whichever whole color has the lower or higher mean
		case 5u: {
			return select(paint, backdrop, dot(backdrop, average) <= dot(paint, average));
		}
		case 10u: {
			return select(paint, backdrop, dot(backdrop, average) >= dot(paint, average));
		}
		default: {
			return vec3<f32>(blend_channel(backdrop.r, paint.r, mode), blend_channel(backdrop.g, paint.g, mode), blend_channel(backdrop.b, paint.b, mode));
		}
	}
}

// The blend mode's mix of straight gamma-encoded colors, matching the CPU's `blend_gamma`
fn blend_gamma(backdrop: vec3<f32>, paint: vec3<f32>, mode: u32) -> vec3<f32> {
	if mode >= 22u && mode <= 25u {
		return blend_components(clamp(backdrop, vec3<f32>(0.), vec3<f32>(1.)), clamp(paint, vec3<f32>(0.), vec3<f32>(1.)), mode);
	}
	return blend(backdrop, paint, mode);
}

// One channel mixed by a separable blend mode, clamped to 0 to 1
fn blend_channel(backdrop: f32, paint: f32, mode: u32) -> f32 {
	var result: f32;
	switch mode {
		// Darken, Multiply, Color Burn, Linear Burn
		case 1u: {
			result = min(paint, backdrop);
		}
		case 2u: {
			result = paint * backdrop;
		}
		case 3u: {
			result = color_burn(backdrop, paint);
		}
		case 4u: {
			result = backdrop + paint - 1.;
		}
		// Lighten, Screen, Color Dodge, Linear Dodge
		case 6u: {
			result = max(paint, backdrop);
		}
		case 7u: {
			result = screen(backdrop, paint);
		}
		case 8u: {
			result = color_dodge(backdrop, paint);
		}
		case 9u: {
			result = backdrop + paint;
		}
		// Overlay, Soft Light, Hard Light, Vivid Light, Linear Light, Pin Light, Hard Mix
		case 11u: {
			result = hard_light(paint, backdrop);
		}
		case 12u: {
			result = soft_light(backdrop, paint);
		}
		case 13u: {
			result = hard_light(backdrop, paint);
		}
		case 14u: {
			result = select(color_dodge(backdrop, 2. * paint - 1.), color_burn(backdrop, 2. * paint), paint <= 0.5);
		}
		case 15u: {
			result = linear_light(backdrop, paint);
		}
		case 16u: {
			result = select(max(2. * paint - 1., backdrop), min(2. * paint, backdrop), paint <= 0.5);
		}
		case 17u: {
			result = select(1., 0., linear_light(backdrop, paint) < 0.5);
		}
		// Difference, Exclusion, Subtract, Divide
		case 18u: {
			result = abs(backdrop - paint);
		}
		case 19u: {
			result = backdrop + paint - 2. * backdrop * paint;
		}
		case 20u: {
			result = backdrop - paint;
		}
		case 21u: {
			// A black backdrop stays black, and any other turns white when divided by black
			result = select(select(backdrop / paint, 1., paint <= BLEND_EDGE_TOLERANCE), 0., backdrop <= BLEND_EDGE_TOLERANCE);
		}
		// Normal, and the alpha-only utility modes, which mix no color
		default: {
			result = paint;
		}
	}
	return clamp(result, 0., 1.);
}

fn screen(backdrop: f32, paint: f32) -> f32 {
	return 1. - (1. - paint) * (1. - backdrop);
}

// How near black or white a channel counts as it to the blend modes with a hard edge there, matching `BLEND_EDGE_TOLERANCE` in core-types
const BLEND_EDGE_TOLERANCE: f32 = 1e-6;

fn color_burn(backdrop: f32, paint: f32) -> f32 {
	if abs(backdrop - 1.) <= BLEND_EDGE_TOLERANCE {
		return 1.;
	}
	if abs(paint) <= BLEND_EDGE_TOLERANCE {
		return 0.;
	}
	return 1. - min((1. - backdrop) / paint, 1.);
}

fn color_dodge(backdrop: f32, paint: f32) -> f32 {
	if abs(paint - 1.) <= BLEND_EDGE_TOLERANCE {
		return 1.;
	}
	return min(backdrop / (1. - paint), 1.);
}

fn hard_light(backdrop: f32, paint: f32) -> f32 {
	return select(screen(2. * paint - 1., backdrop), 2. * paint * backdrop, paint <= 0.5);
}

fn soft_light(backdrop: f32, paint: f32) -> f32 {
	if paint <= 0.5 {
		return backdrop - (1. - 2. * paint) * backdrop * (1. - backdrop);
	}
	let curve = select(sqrt(backdrop), ((16. * backdrop - 12.) * backdrop + 4.) * backdrop, backdrop <= 0.25);
	return backdrop + (2. * paint - 1.) * (curve - backdrop);
}

fn linear_light(backdrop: f32, paint: f32) -> f32 {
	return select(2. * paint - 1. + backdrop, 2. * paint + backdrop - 1., paint <= 0.5);
}

// Hue, Saturation, Color, and Luminosity on gamma-encoded channels, as the compositing specification constructs them
fn blend_components(backdrop: vec3<f32>, paint: vec3<f32>, mode: u32) -> vec3<f32> {
	switch mode {
		case 22u: {
			let hued = set_saturation(paint, channel_range(backdrop));
			return set_luminosity(hued, luma(hued), luma(backdrop));
		}
		case 23u: {
			let saturated = set_saturation(backdrop, channel_range(paint));
			return set_luminosity(saturated, luma(saturated), luma(backdrop));
		}
		case 24u: {
			return set_luminosity(paint, luma(paint), luma(backdrop));
		}
		default: {
			return set_luminosity(backdrop, luma(backdrop), luma(paint));
		}
	}
}

fn luma(color: vec3<f32>) -> f32 {
	return dot(color, vec3<f32>(0.3, 0.59, 0.11));
}

fn channel_range(color: vec3<f32>) -> f32 {
	return max(max(color.r, color.g), color.b) - min(min(color.r, color.g), color.b);
}

fn set_saturation(color: vec3<f32>, saturation: f32) -> vec3<f32> {
	let range = channel_range(color);
	if range <= 0. {
		return vec3<f32>(0.);
	}
	return (color - min(min(color.r, color.g), color.b)) * (saturation / range);
}

// Shifts the channels from `from_luma` to `luminosity`, then pulls them toward it just enough to bring every channel back into 0 to 1
fn set_luminosity(color: vec3<f32>, from_luma: f32, luminosity: f32) -> vec3<f32> {
	var channels = color + (luminosity - from_luma);

	let low = min(min(channels.r, channels.g), channels.b);
	if low < 0. {
		channels = luminosity + (channels - luminosity) * (luminosity / (luminosity - low));
	}
	let high = max(max(channels.r, channels.g), channels.b);
	if high > 1. {
		channels = luminosity + (channels - luminosity) * ((1. - luminosity) / (high - luminosity));
	}

	return clamp(channels, vec3<f32>(0.), vec3<f32>(1.));
}

@fragment
fn scale(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return textureLoad(source, vec2<i32>(position.xy), 0) * parameters.amount;
}

// A mipmap level from the one above it, each texel averaging a block of two by two, which grows to three along an odd side's last texel
@fragment
fn halve(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let size = vec2<i32>(textureDimensions(source));
	let half_size = max(size / 2, vec2<i32>(1));
	let index = vec2<i32>(position.xy);
	let start = index * 2;
	// Selecting per axis, since WebGL2 can't select between integer vectors
	let last = index + 1 == half_size;
	let end = clamp(vec2<i32>(select(start.x + 2, size.x, last.x), select(start.y + 2, size.y, last.y)), start + 1, size);

	var sum = vec4<f32>(0.);
	for (var y = start.y; y < end.y; y++) {
		for (var x = start.x; x < end.x; x++) {
			sum += textureLoad(source, vec2<i32>(x, y), 0);
		}
	}
	let count = end - start;
	return sum / f32(count.x * count.y);
}

@fragment
fn decode_to_linear(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let color = textureLoad(source, vec2<i32>(position.xy), 0);
	if color.a <= 0. {
		return vec4<f32>(0.);
	}

	return vec4<f32>(srgb_to_linear(max(color.rgb / color.a, vec3<f32>(0.))) * color.a, color.a);
}

@fragment
fn encode_to_gamma(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let color = textureLoad(source, vec2<i32>(position.xy), 0);
	if color.a <= 0. {
		return vec4<f32>(0.);
	}

	return vec4<f32>(linear_to_srgb(max(color.rgb / color.a, vec3<f32>(0.))) * color.a, color.a);
}

fn srgb_to_linear(srgb: vec3<f32>) -> vec3<f32> {
	let low = srgb / 12.92;
	let high = pow((srgb + 0.055) / 1.055, vec3<f32>(2.4));
	return select(high, low, srgb <= vec3<f32>(0.04045));
}

fn linear_to_srgb(linear: vec3<f32>) -> vec3<f32> {
	let low = linear * 12.92;
	let high = 1.055 * pow(linear, vec3<f32>(1. / 2.4)) - 0.055;
	return select(high, low, linear <= vec3<f32>(0.0031308));
}

// The premultiplied color everywhere, as a solid paint overlay paints
@fragment
fn flood(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return parameters.color;
}

// A tinted image cut to the full-strength `content`, for a piece within the content drawn in a layer of its own
@fragment
fn mask_within(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	return textureLoad(source, pixel, 0) * textureLoad(content, pixel, 0).a;
}
