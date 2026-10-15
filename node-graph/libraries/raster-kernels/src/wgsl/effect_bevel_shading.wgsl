// How far the light brightens the bevel's surface past a flat surface's brightness, or if `shadow`, how far it falls short.
// `slope` scales the height's gradient into the surface's steepness, and is negative for a bevel pressed into the surface.
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let gradient = vec2<f32>(
		input_0_load(coord + vec2<i32>(1, 0)).a - input_0_load(coord - vec2<i32>(1, 0)).a,
		input_0_load(coord + vec2<i32>(0, 1)).a - input_0_load(coord - vec2<i32>(0, 1)).a,
	) / 2.;
	let light = vec3<f32>(parameters.light_x, parameters.light_y, parameters.light_z);
	let shade = dot(normalize(vec3<f32>(-parameters.slope * gradient, 1.)), light);

	if parameters.shadow {
		return vec4<f32>(clamp((light.z - shade) / max(light.z, 1e-4), 0., 1.));
	}
	return vec4<f32>(clamp((shade - light.z) / max(1. - light.z, 1e-4), 0., 1.));
}
