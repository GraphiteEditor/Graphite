struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
	var out: VertexOutput;

	let pos = array(
		// 1st triangle
		vec2f( -1.0, -1.0), // center
		vec2f( 1.0, -1.0), // right, center
		vec2f( -1.0, 1.0), // center, top

		// 2nd triangle
		vec2f( -1.0, 1.0), // center, top
		vec2f( 1.0, -1.0), // right, center
		vec2f( 1.0, 1.0), // right, top
	);
	let xy = pos[vertex_index];
	out.clip_position = vec4f(xy , 0.0, 1.0);
	let coords = (xy / 2. + 0.5);
	out.tex_coords = vec2f(coords.x, 1. - coords.y);
	return out;
}


struct Constants {
	viewport_scale: vec2<f32>,
	viewport_offset: vec2<f32>,
};

var<push_constant> constants: Constants;

@group(0) @binding(0)
var t_viewport: texture_2d<f32>;
@group(0) @binding(1)
var t_overlays: texture_2d<f32>;
@group(0) @binding(2)
var t_ui: texture_2d<f32>;
@group(0) @binding(3)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let ui_raw = textureSample(t_ui, s_diffuse, in.tex_coords);
	if (ui_raw.a >= 0.999) {
		return ui_raw;
	}

	let viewport_coordinate = (in.tex_coords - constants.viewport_offset) * constants.viewport_scale;

	// Vello renders its values to an `RgbaUnorm` texture, but if we try to use this in the main rendering pipeline
	// which renders to an `Srgb` surface, gamma mapping is applied twice. This converts back to linear to compensate.
	let overlay = srgb_to_linear(textureSample(t_overlays, s_diffuse, viewport_coordinate));
	let viewport = srgb_to_linear(textureSample(t_viewport, s_diffuse, viewport_coordinate));

	// UI texture is premultiplied, we need to unpremultiply before blending
	let ui = unpremultiply(ui_raw);

	if (overlay.a < 0.001) {
		return blend_in_srgb(ui, viewport);
	}

	let composite = blend_in_srgb(overlay, viewport);
	return blend_in_srgb(ui, composite);
}

fn blend_in_srgb(
  fg: vec4<f32>, bg: vec4<f32>,
) -> vec4<f32> {
	let bg_srgb = linear_to_srgb(bg);
	let fg_srgb = linear_to_srgb(fg);
	let out_srgb = blend(fg_srgb, bg_srgb);
	return srgb_to_linear(out_srgb);
}

fn blend(fg: vec4<f32>, bg: vec4<f32>) -> vec4<f32> {
	let a = fg.a + bg.a * (1.0 - fg.a);
	let rgb = fg.rgb * fg.a + bg.rgb * bg.a * (1.0 - fg.a);
	return vec4<f32>(rgb, a);
}

fn unpremultiply(in: vec4<f32>) -> vec4<f32> {
	if (in.a > 0.0) {
		return vec4<f32>((in.rgb / in.a), in.a);
	} else {
		return vec4<f32>(0.0);
	}
}

fn linear_to_srgb(in: vec4<f32>) -> vec4<f32> {
	let cutoff = vec3<f32>(0.0031308);
	let lo = in.rgb * 12.92;
	let hi = 1.055 * pow(max(in.rgb, vec3<f32>(0.0)), vec3<f32>(1.0/2.4)) - 0.055;
	return vec4<f32>(select(lo, hi, in.rgb > cutoff), in.a);
}

fn srgb_to_linear(in: vec4<f32>) -> vec4<f32> {
	let cutoff = vec3<f32>(0.04045);
	let lo = in.rgb / 12.92;
	let hi = pow((in.rgb + 0.055) / 1.055, vec3<f32>(2.4));
	return vec4<f32>(select(lo, hi, in.rgb > cutoff), in.a);
}
