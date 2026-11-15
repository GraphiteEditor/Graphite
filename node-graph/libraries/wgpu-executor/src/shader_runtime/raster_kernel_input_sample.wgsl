fn INPUT_sample(texel: vec2<f32>) -> vec4<f32> {
	return textureSampleLevel(INPUT_texture, raster_kernel_sampler, texel / vec2<f32>(INPUT_size()), 0.);
}
