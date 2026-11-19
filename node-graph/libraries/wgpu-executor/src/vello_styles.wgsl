// The passes of the layer styles' finer options, compiled together with `vello_effects.wgsl` and sharing its inputs and helpers

// The last of the gradient's `GRADIENT_TABLE_SIZE` samples
const LAST_GRADIENT_SAMPLE: f32 = 255.;

// A one-row table's value at a position from 0 to 1, interpolated between its two nearest entries
fn curve_lookup(table: texture_2d<f32>, value: f32) -> vec4<f32> {
	let last = f32(textureDimensions(table).x - 1u);
	let position = clamp(value, 0., 1.) * last;
	let index = min(floor(position), last - 1.);

	let before = textureLoad(table, vec2<i32>(i32(index), 0), 0);
	let after = textureLoad(table, vec2<i32>(i32(index) + 1, 0), 0);
	return mix(before, after, position - index);
}

// Where each texel lies across an outline for a contour gradient starting `values.x` texels past the edge and spanning `values.y` inward,
// from the distances out from the edge (`source`) and in from it (`secondary`), rounded to the gradient's 256 samples, matching `contour_gradient_position`
@fragment
fn contour_gradient_position(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let outward = textureLoad(source, pixel, 0).r;
	let inward = textureLoad(secondary, pixel, 0).r;

	// Each texel's center lies half a texel short of its distance from the edge, inside or out
	let signed_distance = max(outward - 0.5, 0.) - max(inward - 0.5, 0.);
	return vec4<f32>(floor(clamp((parameters.values.x - signed_distance) / parameters.values.y, 0., 1.) * LAST_GRADIENT_SAMPLE + 0.5) / LAST_GRADIENT_SAMPLE);
}

// The band's coverage (`source`) of a premultiplied paint (`secondary`)
@fragment
fn paint_band(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	return textureLoad(secondary, pixel, 0) * textureLoad(source, pixel, 0).r;
}

// The band's coverage (`content`) of the gradient `secondary` holds in a row of straight colors from its start to its end, at each texel's place across the outline (`source`),
// faded by the opacity `amount`, matching the CPU's `contour_gradient_colors`
@fragment
fn contour_gradient_band(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let place = textureLoad(source, pixel, 0).r;

	// The gradient's samples span its ends, and each place lies on one of them
	let last = i32(textureDimensions(secondary).x) - 1;
	let color = textureLoad(secondary, vec2<i32>(clamp(i32(round(place * f32(last))), 0, last), 0), 0);

	let alpha = color.a * parameters.amount * textureLoad(content, pixel, 0).r;
	return vec4<f32>(color.rgb * alpha, alpha);
}

// A texture's value at `offset` from `pixel` toward its right and lower neighbors, interpolated between them and holding the edge texels beyond the edges, matching the CPU's `sample_between`
fn texel_between(texture: texture_2d<f32>, pixel: vec2<i32>, offset: vec2<f32>) -> vec4<f32> {
	let last = vec2<i32>(textureDimensions(texture)) - 1;
	if all(offset == vec2<f32>(0.)) {
		return textureLoad(texture, clamp(pixel, vec2<i32>(0), last), 0);
	}

	let top = mix(textureLoad(texture, clamp(pixel, vec2<i32>(0), last), 0), textureLoad(texture, clamp(pixel + vec2<i32>(1, 0), vec2<i32>(0), last), 0), offset.x);
	let bottom = mix(textureLoad(texture, clamp(pixel + vec2<i32>(0, 1), vec2<i32>(0), last), 0), textureLoad(texture, clamp(pixel + vec2<i32>(1, 1), vec2<i32>(0), last), 0), offset.x);
	return mix(top, bottom, offset.y);
}

// A bevel's surface in units of its span at `offset` from `pixel`, from its height (`source`, centered on the edge) reshaped by the contour, if `taps[1].x` is 1, then sunk by the texture's gray.
// Matches the CPU's `bevel_shading`.
// The contour, a row in `secondary`, spans the range `taps[0].y` down from an inner bevel's top (`taps[0].x` 0), up from an outer bevel's foot (1), or an emboss's whole slope (2), continued past its ends along the slopes `taps[0].zw`.
// The texture is the premultiplied `content`, sinking the surface by its gray times `taps[1].z`.
fn bevel_surface_at(pixel: vec2<i32>, offset: vec2<f32>) -> f32 {
	let height = texel_between(source, pixel, offset).r + 0.5;
	var surface = height;

	let shape = parameters.taps[0];
	if parameters.taps[1].x > 0.5 {
		var input = height;
		if shape.x < 0.5 {
			input = 1. - (1. - height) / shape.y;
		} else if shape.x < 1.5 {
			input = height / shape.y;
		}

		let last = i32(textureDimensions(secondary).x) - 1;
		if input < 0. {
			surface = textureLoad(secondary, vec2<i32>(0, 0), 0).r + shape.z * input;
		} else if input > 1. {
			surface = textureLoad(secondary, vec2<i32>(last, 0), 0).r + shape.w * (input - 1.);
		} else {
			surface = curve_lookup(secondary, input).r;
		}
	}

	if parameters.taps[1].z == 0. {
		return surface;
	}
	let texture = texel_between(content, pixel, offset).rgb;
	return surface - parameters.taps[1].z * dot(texture, vec3<f32>(0.299, 0.587, 0.114));
}

// How much brighter than flat ground the light toward `values.xyz` lights a bevel's surface rising `values.w` texels for each unit, negative where darker, matching the CPU's `bevel_shading`.
// Its slope is the difference across a texel's neighbors averaged over three rows or columns, and with `taps[1].y` 1 the contour's nine samples toward the right and lower neighbors are averaged.
// With `taps[1].w` 1 or 2, it's instead the highlight or shadow that makes, for a bevel with no gloss contour or fade at the foot.
@fragment
fn bevel_lit(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let samples = select(1, 3, parameters.taps[1].y > 0.5);

	var total = 0.;
	for (var sample_y = 0; sample_y < samples; sample_y++) {
		for (var sample_x = 0; sample_x < samples; sample_x++) {
			let offset = vec2<f32>(f32(sample_x), f32(sample_y)) / 3.;

			// One call site, since FXC inlines every call, which slows the compile at the first draw
			var across = 0.;
			var down = 0.;
			for (var neighbor_y = -1; neighbor_y <= 1; neighbor_y++) {
				for (var neighbor_x = -1; neighbor_x <= 1; neighbor_x++) {
					if neighbor_x != 0 || neighbor_y != 0 {
						let surface = bevel_surface_at(pixel + vec2<i32>(neighbor_x, neighbor_y), offset);
						across += f32(neighbor_x) * surface;
						down += f32(neighbor_y) * surface;
					}
				}
			}

			let gradient = vec2<f32>(across, down) * parameters.values.w / 6.;
			total += clamp(dot(normalize(vec3<f32>(-gradient, 1.)), parameters.values.xyz), 0., 1.);
		}
	}

	let brighter = total / f32(samples * samples) - parameters.values.z;
	if parameters.taps[1].w > 0.5 {
		return vec4<f32>(bevel_amount_of(brighter, parameters.values.z, parameters.taps[1].w > 1.5));
	}
	return vec4<f32>(brighter);
}

// A bevel's highlight, or if `shadow` its shadow, from how much brighter than a flat surface's brightness `light_z` it's lit
fn bevel_amount_of(brighter: f32, light_z: f32, shadow: bool) -> f32 {
	if shadow {
		return select(0., clamp(-brighter / light_z, 0., 1.), light_z > 0.);
	}
	return select(0., clamp(brighter / (1. - light_z), 0., 1.), light_z < 1.);
}

// A bevel's highlight, or with `values.w` 1 its shadow, from how much brighter than a flat surface's brightness `values.x` it's lit (`source`), matching the CPU's `bevel_shading`.
// With `values.y` 1, the gloss contour `secondary` holds in a row reshapes how lit it is first, averaged over nine samples toward the right and lower neighbors with `values.z` 1.
// With `taps[0].x` 1, it reaches no more than the height (`content`, centered on the edge) above the foot divided by `amount`, fading out across the slopes' foot.
@fragment
fn bevel_amount(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let light_z = parameters.values.x;
	var brighter = textureLoad(source, pixel, 0).r;
	if parameters.values.y > 0.5 {
		let samples = select(1, 3, parameters.values.z > 0.5);
		var total = 0.;
		for (var sample_y = 0; sample_y < samples; sample_y++) {
			for (var sample_x = 0; sample_x < samples; sample_x++) {
				total += curve_lookup(secondary, texel_between(source, pixel, vec2<f32>(f32(sample_x), f32(sample_y)) / 3.).r + light_z).r;
			}
		}
		brighter = total / f32(samples * samples) - light_z;
	}

	var amount = bevel_amount_of(brighter, light_z, parameters.values.w > 0.5);
	if parameters.taps[0].x > 0.5 {
		amount = min(amount, (textureLoad(content, pixel, 0).r + 0.5) / parameters.amount);
	}
	return vec4<f32>(amount);
}
