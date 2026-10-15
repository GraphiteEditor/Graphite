// The passes that carry out an item's effects on the pixels of its content, which are premultiplied with gamma-encoded color like the pixels Vello GPU blends

struct Parameters {
	// Converts a target pixel's position into the normalized coordinates the inputs are sampled at
	target_to_uv: vec2<f32>,
	// One step along the axis a kernel runs, in normalized coordinates
	step: vec2<f32>,
	// How far a silhouette is cast, in normalized coordinates
	offset: vec2<f32>,
	// The factor the scale pass multiplies by
	amount: f32,
	tap_count: u32,
	// The premultiplied color a silhouette is tinted
	color: vec4<f32>,
	// The extra constants a pass needs beyond the fields above
	values: vec4<f32>,
	// Each kernel tap's distance in steps (x) and weight (y)
	taps: array<vec4<f32>, 25>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var secondary: texture_2d<f32>;
@group(0) @binding(2) var content: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var<uniform> parameters: Parameters;

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

// Grows the shapes by one step along an axis, so steps that double each time grow them by any distance in a few passes
@fragment
fn dilate(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let uv = position.xy * parameters.target_to_uv;
	return max(sample_source(uv), max(sample_source(uv - parameters.step), sample_source(uv + parameters.step)));
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

// The coverage of where a silhouette doesn't reach, which turns an inner glow from the edges into one from the center
@fragment
fn invert_coverage(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return vec4<f32>(1. - textureLoad(source, vec2<i32>(position.xy), 0).r);
}

// Satin's shading from two casts of the silhouette: how much one covers without the other, or how much they agree with `amount` 1
@fragment
fn satin(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let difference = abs(textureLoad(source, pixel, 0).r - textureLoad(secondary, pixel, 0).r);
	return vec4<f32>(select(difference, 1. - difference, parameters.amount > 0.5));
}

// Jump flooding keeps each texel's offset to the nearest seed found so far (xy) and whether one is found (z).
// The seeds are the texels inside the content's silhouette, or outside it with `amount` 1, and each is its own nearest.
@fragment
fn jump_flood_seed(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let inside = textureLoad(source, vec2<i32>(position.xy), 0).a >= 0.5;
	let seed = inside != (parameters.amount > 0.5);
	return vec4<f32>(0., 0., select(0., 1., seed), 0.);
}

// Takes the nearest of the seeds that the texel and its eight neighbors `values.x` texels away have found so far
@fragment
fn jump_flood_step(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let size = vec2<i32>(textureDimensions(source));
	let step = i32(parameters.values.x);

	var best = textureLoad(source, pixel, 0);
	for (var neighbor_y = -1; neighbor_y <= 1; neighbor_y++) {
		for (var neighbor_x = -1; neighbor_x <= 1; neighbor_x++) {
			let direction = vec2<i32>(neighbor_x, neighbor_y);
			let neighbor = pixel + direction * step;
			if any(neighbor < vec2<i32>(0)) || any(neighbor >= size) {
				continue;
			}
			let found = textureLoad(source, neighbor, 0);
			if found.z < 0.5 {
				continue;
			}

			let candidate = found.xy + vec2<f32>(direction * step);
			if best.z < 0.5 || dot(candidate, candidate) < dot(best.xy, best.xy) {
				best = vec4<f32>(candidate, 1., 0.);
			}
		}
	}
	return best;
}

// Covers the texels within `values.x` texels of the edge, which lies about half a texel short of the nearest seed, fading out over one texel
@fragment
fn edge_band(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let nearest = textureLoad(source, vec2<i32>(position.xy), 0);
	if nearest.z < 0.5 {
		return vec4<f32>(0.);
	}
	return vec4<f32>(clamp(parameters.values.x + 1. - length(nearest.xy), 0., 1.));
}

// A bevel's height from 0 to 1, shaped by the style in `values.y` over a slope `values.x` texels wide, from the nearest texels inside (source) and outside (secondary)
@fragment
fn bevel_height(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let to_inside = textureLoad(source, pixel, 0);
	let to_outside = textureLoad(secondary, pixel, 0);
	let size = parameters.values.x;

	// Texels inside the silhouette are those at no distance from it, and the edge lies about half a texel short of the nearest texel across it
	let far = 4. * size + 4.;
	var signed_distance: f32;
	if to_inside.z > 0.5 && dot(to_inside.xy, to_inside.xy) == 0. {
		signed_distance = select(far, length(to_outside.xy) - 0.5, to_outside.z > 0.5);
	} else {
		signed_distance = select(-far, 0.5 - length(to_inside.xy), to_inside.z > 0.5);
	}

	var progress: f32;
	switch u32(parameters.values.y) {
		case 0u: {
			progress = signed_distance / size;
		}
		case 1u: {
			progress = (signed_distance + size) / size;
		}
		case 2u: {
			progress = (signed_distance + size / 2.) / size;
		}
		default: {
			progress = abs(signed_distance) / (size / 2.);
		}
	}
	let height = clamp(progress, 0., 1.);
	return vec4<f32>(height * height * (3. - 2. * height));
}

fn height_at(pixel: vec2<i32>) -> f32 {
	return textureLoad(source, clamp(pixel, vec2<i32>(0), vec2<i32>(textureDimensions(source)) - 1), 0).r;
}

// How far the light toward `values.yzw` brightens the bevel's surface past a flat surface's brightness, or with `amount` 1, how far it falls short.
// `values.x` scales the height's gradient into the surface's steepness, and is negative for a bevel pressed into the surface.
@fragment
fn bevel_shading(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let gradient = vec2<f32>(
		height_at(pixel + vec2<i32>(1, 0)) - height_at(pixel - vec2<i32>(1, 0)),
		height_at(pixel + vec2<i32>(0, 1)) - height_at(pixel - vec2<i32>(0, 1)),
	) / 2.;
	let light = parameters.values.yzw;
	let shade = dot(normalize(vec3<f32>(-parameters.values.x * gradient, 1.)), light);

	if parameters.amount > 0.5 {
		return vec4<f32>(clamp((light.z - shade) / max(light.z, 1e-4), 0., 1.));
	}
	return vec4<f32>(clamp((shade - light.z) / max(1. - light.z, 1e-4), 0., 1.));
}

// A silhouette's coverage in its color
@fragment
fn tint(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return parameters.color * textureLoad(source, vec2<i32>(position.xy), 0).r;
}

// Stacks a tinted silhouette over the shadows and glows beneath the content
@fragment
fn accumulate(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let beneath = textureLoad(source, pixel, 0);
	let tinted = parameters.color * textureLoad(secondary, pixel, 0).r;

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

// Places the shadows and glows beneath the image, knocked out by the content at full strength so a faded fill doesn't reveal them
@fragment
fn composite_beneath(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let image = textureLoad(source, pixel, 0);
	let beneath = textureLoad(secondary, pixel, 0);
	let full_content = textureLoad(content, pixel, 0);

	return image + beneath * (1. - full_content.a);
}

@fragment
fn scale(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	return textureLoad(source, vec2<i32>(position.xy), 0) * parameters.amount;
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
