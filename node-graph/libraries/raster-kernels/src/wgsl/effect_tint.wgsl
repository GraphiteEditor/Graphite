// A silhouette's coverage in its premultiplied gamma-encoded color
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	return params().color * input_0_load(coord).a;
}
