// Takes the nearest of the seeds that the texel and its eight neighbors `step` texels away have found so far
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let step = params().step;

	var best = input_0_load(coord);
	for (var neighbor_y = -1; neighbor_y <= 1; neighbor_y++) {
		for (var neighbor_x = -1; neighbor_x <= 1; neighbor_x++) {
			let direction = vec2<i32>(neighbor_x, neighbor_y);
			let neighbor = coord + direction * step;
			if any(neighbor < vec2<i32>(0)) || any(neighbor >= input_0_size()) {
				continue;
			}
			let found = input_0_load(neighbor);
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
