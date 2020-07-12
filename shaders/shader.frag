#version 450

layout(location=0) in vec2 v_uv;

layout(location=0) out vec4 f_color;

struct Dimensions_u32 { uint width; uint height; };
struct Corners_f32 { float top_left; float top_right; float bottom_right; float bottom_left; };
struct Sides_f32 { float top; float right; float bottom; float left; };

layout(set=0, binding=0) uniform GuiNodeUniform {
	Dimensions_u32 dimensions;
	Corners_f32 corners_radius;
	Sides_f32 sides_inset;
	float border_thickness;
	vec4 border_color;
	vec4 fill_color;
};

layout(set=0, binding=1) uniform texture2D t_texture;
layout(set=0, binding=2) uniform sampler s_texture;

void main() {
	f_color = fill_color * texture(sampler2D(t_texture, s_texture), v_uv / textureSize(sampler2D(t_texture, s_texture), 0) * 500);
}
