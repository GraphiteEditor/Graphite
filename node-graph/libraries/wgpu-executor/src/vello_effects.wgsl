// The passes that carry out an item's effects on the pixels of its content, which are premultiplied with gamma-encoded color like the pixels Vello GPU blends

struct Parameters {
	// Converts a target pixel's position into the normalized coordinates the inputs are sampled at
	target_to_uv: vec2<f32>,
	// One step along the axis a kernel runs, in normalized coordinates
	step: vec2<f32>,
	// How far a silhouette is cast, in normalized coordinates
	offset: vec2<f32>,
	// The factor the scale pass multiplies by, or the fill the content is faded by
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

// The coverage of the content's silhouette, shifted by the offset
@fragment
fn cast_silhouette(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return vec4<f32>(sample_source(position.xy * parameters.target_to_uv - parameters.offset).a);
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
fn is_seed(value: f32) -> bool {
	if parameters.values.y > 0.5 {
		// Only those up to 3 texels past it, a band any path in from farther crosses
		return value > parameters.values.z && value <= parameters.values.z + 3.;
	}
	return value > 0.;
}

fn seed_distance(pixel: vec2<i32>, offset: vec2<f32>) -> f32 {
	let value = textureLoad(content, pixel - vec2<i32>(offset), 0).r;
	if parameters.values.y > 0.5 {
		return chamfer_length(offset) + parameters.values.z - value;
	}

	return chamfer_length(offset) + (0.5 - value) + 0.5;
}

@fragment
fn jump_flood_seed(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let seed = is_seed(textureLoad(content, vec2<i32>(position.xy), 0).r);
	return vec4<f32>(0., 0., select(0., 1., seed), 0.);
}

// Takes the nearest of the seeds that the texel and its eight neighbors `values.x` texels away have found so far
@fragment
fn jump_flood_step(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let size = vec2<i32>(textureDimensions(source));
	let step = i32(parameters.values.x);

	var best = textureLoad(source, pixel, 0);
	var best_distance = FAR;
	if best.z > 0.5 {
		best_distance = seed_distance(pixel, best.xy);
	}

	for (var neighbor_y = -1; neighbor_y <= 1; neighbor_y++) {
		for (var neighbor_x = -1; neighbor_x <= 1; neighbor_x++) {
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

// Places a tinted silhouette over the image, within the content at full strength so it stays visible over a faded fill
@fragment
fn composite_within(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let image = textureLoad(source, pixel, 0);
	let tinted = parameters.color * textureLoad(secondary, pixel, 0).r;
	let full_content = textureLoad(content, pixel, 0);

	return tinted * full_content.a + image * (1. - tinted.a);
}

// Places the shadows and glows beneath the image
@fragment
fn composite_beneath(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let image = textureLoad(source, pixel, 0);
	let beneath = textureLoad(secondary, pixel, 0);

	return image + beneath * (1. - image.a);
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
