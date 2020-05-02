#version 450

layout(location=0) in vec2 v_uv;

layout(location=0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D t_texture;

void main() {
	// f_color = texture(t_texture, v_uv / textureSize(t_texture, 0) * 100);
	f_color = vec4(0.0, 1.0, 0.0, 1.0);
}