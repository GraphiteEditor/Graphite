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

// The image (source) with the paint (secondary input) mixed over its colors by the blend mode in `values.x`, as much as the paint's alpha times `amount`, keeping the image's alpha
@fragment
fn paint_overlay(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let image = textureLoad(source, pixel, 0);
	let paint = textureLoad(secondary, pixel, 0);
	if image.a <= 0. {
		return vec4<f32>(0.);
	}
	if paint.a <= 0. {
		return image;
	}

	// The blend modes work on straight linear colors, as the CPU's do
	let backdrop = srgb_to_linear(image.rgb / image.a);
	let blended = blend(backdrop, srgb_to_linear(paint.rgb / paint.a), u32(parameters.values.x));
	let mixed = mix(backdrop, blended, parameters.amount * paint.a);

	return vec4<f32>(linear_to_srgb(mixed) * image.a, image.a);
}

// The paint mixed with the backdrop by a blend mode, numbered in the order `BlendMode` declares them, matching `apply_blend_mode`
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
		// The component modes are defined on gamma-encoded channels
		case 22u, 23u, 24u, 25u: {
			return srgb_to_linear(blend_components(clamp(linear_to_srgb(backdrop), vec3<f32>(0.), vec3<f32>(1.)), clamp(linear_to_srgb(paint), vec3<f32>(0.), vec3<f32>(1.)), mode));
		}
		default: {
			return vec3<f32>(blend_channel(backdrop.r, paint.r, mode), blend_channel(backdrop.g, paint.g, mode), blend_channel(backdrop.b, paint.b, mode));
		}
	}
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
			result = select(color_dodge(2. * paint - 1., backdrop), color_burn(2. * paint, backdrop), paint <= 0.5);
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
			result = select(backdrop / paint, 1., backdrop == 0.);
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

fn color_burn(backdrop: f32, paint: f32) -> f32 {
	if backdrop == 1. {
		return 1.;
	}
	if paint == 0. {
		return 0.;
	}
	return 1. - min((1. - backdrop) / paint, 1.);
}

fn color_dodge(backdrop: f32, paint: f32) -> f32 {
	if paint == 1. {
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

// ===========
// Adjustments
// ===========

// The adjustment passes work on a pixel's straight color, leaving fully transparent pixels empty since they keep no color to adjust

// A one-row table's value at a position from 0 to 1, interpolated between its two nearest entries
fn curve_lookup(table: texture_2d<f32>, value: f32) -> vec4<f32> {
	let last = f32(textureDimensions(table).x - 1u);
	let position = clamp(value, 0., 1.) * last;
	let index = min(floor(position), last - 1.);

	let before = textureLoad(table, vec2<i32>(i32(index), 0), 0);
	let after = textureLoad(table, vec2<i32>(i32(index) + 1, 0), 0);
	return mix(before, after, position - index);
}

// Each channel through its own curve
@fragment
fn adjust_curves(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = textureLoad(source, vec2<i32>(position.xy), 0);
	if pixel.a <= 0. {
		return vec4<f32>(0.);
	}
	let color = vec4<f32>(pixel.rgb / pixel.a, pixel.a);

	let red = curve_lookup(secondary, color.r).r;
	let green = curve_lookup(secondary, color.g).g;
	let blue = curve_lookup(secondary, color.b).b;
	let alpha = curve_lookup(secondary, color.a).a;
	return vec4<f32>(vec3<f32>(red, green, blue) * alpha, alpha);
}

// The cube entry at a grid point, from the texture's tiles of slices of constant blue, each with red across and green down
fn cube_entry(size: u32, point: vec3<u32>) -> vec3<f32> {
	let columns = textureDimensions(secondary).x / size;
	let tile = vec2<u32>(point.b % columns, point.b / columns) * size;
	return textureLoad(secondary, vec2<i32>(tile + point.rg), 0).rgb;
}

// The cube's color at a point, interpolated in the tetrahedron of its cell that holds it, which fits the cube far closer than trilinear interpolation.
// The cube has `values.x` samples along each side.
fn cube_lookup(color: vec3<f32>) -> vec3<f32> {
	let size = u32(parameters.values.x);
	let scaled = clamp(color, vec3<f32>(0.), vec3<f32>(1.)) * f32(size - 1u);
	let base = min(vec3<u32>(floor(scaled)), vec3<u32>(size - 2u));
	let f = scaled - vec3<f32>(base);

	let black = cube_entry(size, base);
	let white = cube_entry(size, base + vec3<u32>(1u, 1u, 1u));
	let r = cube_entry(size, base + vec3<u32>(1u, 0u, 0u));
	let g = cube_entry(size, base + vec3<u32>(0u, 1u, 0u));
	let b = cube_entry(size, base + vec3<u32>(0u, 0u, 1u));
	let rg = cube_entry(size, base + vec3<u32>(1u, 1u, 0u));
	let rb = cube_entry(size, base + vec3<u32>(1u, 0u, 1u));
	let gb = cube_entry(size, base + vec3<u32>(0u, 1u, 1u));

	// Each tetrahedron runs from the cell's black corner to its white one, stepping along the axes in order of how far the point lies along them
	if f.r >= f.g && f.g >= f.b {
		return (1. - f.r) * black + (f.r - f.g) * r + (f.g - f.b) * rg + f.b * white;
	} else if f.r >= f.b && f.b >= f.g {
		return (1. - f.r) * black + (f.r - f.b) * r + (f.b - f.g) * rb + f.g * white;
	} else if f.b >= f.r && f.r >= f.g {
		return (1. - f.b) * black + (f.b - f.r) * b + (f.r - f.g) * rb + f.g * white;
	} else if f.g >= f.r && f.r >= f.b {
		return (1. - f.g) * black + (f.g - f.r) * g + (f.r - f.b) * rg + f.b * white;
	} else if f.g >= f.b && f.b >= f.r {
		return (1. - f.g) * black + (f.g - f.b) * g + (f.b - f.r) * gb + f.r * white;
	}
	return (1. - f.b) * black + (f.b - f.g) * b + (f.g - f.r) * gb + f.r * white;
}

// The color through the cube and alpha through its own curve
@fragment
fn adjust_cube(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = textureLoad(source, vec2<i32>(position.xy), 0);
	if pixel.a <= 0. {
		return vec4<f32>(0.);
	}

	let alpha = curve_lookup(content, pixel.a).a;
	return vec4<f32>(cube_lookup(pixel.rgb / pixel.a) * alpha, alpha);
}

// The color the pixel's luma picks from a table, with the pixel's alpha
@fragment
fn adjust_by_luma(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = textureLoad(source, vec2<i32>(position.xy), 0);
	if pixel.a <= 0. {
		return vec4<f32>(0.);
	}

	let luma = dot(pixel.rgb / pixel.a, parameters.values.rgb);
	return vec4<f32>(curve_lookup(secondary, luma).rgb * pixel.a, pixel.a);
}

fn wrap_positive(value: f32, modulus: f32) -> f32 {
	return value - floor(value / modulus) * modulus;
}

// Hexagon hue in degrees, 0 for gray
fn hexagon_hue_degrees(color: vec3<f32>) -> f32 {
	let maximum = max(color.r, max(color.g, color.b));
	let chroma = maximum - min(color.r, min(color.g, color.b));
	if chroma <= 0. {
		return 0.;
	}

	if maximum == color.r {
		return wrap_positive((color.g - color.b) / chroma, 6.) * 60.;
	} else if maximum == color.g {
		return ((color.b - color.r) / chroma + 2.) * 60.;
	}
	return ((color.r - color.g) / chroma + 4.) * 60.;
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

// Vibrance on linear channels as the chroma factor about the maximum channel (x) and the brightness multiplier (y), mirroring the CPU kernel
fn vibrance_factors(color: vec3<f32>, amount: f32) -> vec2<f32> {
	let maximum = max(color.r, max(color.g, color.b));
	let minimum = min(color.r, min(color.g, color.b));
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

	let protection = skin_tone_window(hexagon_hue_degrees(color)) * (1. - saturation * saturation);
	let protected_amount = amount * (1. - protection * (1. - amount));
	let boost = (5. / 6.) * (1. - 0.4857 * protection) * protected_amount * ratio * (1. - minimum) * rolloff;
	return vec2<f32>(1. / (1. - boost), 1. + protected_amount * brightness / 4.);
}

// Vibrance (amount in values.x) then saturation (scale in values.y), both in linear light
@fragment
fn adjust_vibrance(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = textureLoad(source, vec2<i32>(position.xy), 0);
	if pixel.a <= 0. {
		return vec4<f32>(0.);
	}
	let linear = srgb_to_linear(max(pixel.rgb / pixel.a, vec3<f32>(0.)));

	let maximum = max(linear.r, max(linear.g, linear.b));
	let factors = vibrance_factors(linear, parameters.values.x);
	let vibrant = clamp(factors.y * (maximum + factors.x * (linear - maximum)), vec3<f32>(0.), vec3<f32>(1.));

	// Saturation scales each channel's distance from a gray weighted by ProPhoto's luminance coefficients
	let gray = dot(vibrant, vec3<f32>(0.288040, 0.711874, 0.000086));
	let saturated = clamp(gray + (vibrant - gray) * parameters.values.y, vec3<f32>(0.), vec3<f32>(1.));
	return vec4<f32>(linear_to_srgb(saturated) * pixel.a, pixel.a);
}
