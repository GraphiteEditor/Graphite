fn total_order_key(value: vec4<f32>) -> vec4<u32> {
	let bits = bitcast<vec4<u32>>(value);
	return select(~bits, bits ^ vec4<u32>(0x80000000u), bits < vec4<u32>(0x80000000u));
}

fn from_total_order_key(key: vec4<u32>) -> vec4<f32> {
	return bitcast<vec4<f32>>(select(~key, key ^ vec4<u32>(0x80000000u), key >= vec4<u32>(0x80000000u)));
}

fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let radius = i32(params().radius);
	let start = max(coord - radius, vec2<i32>(0));
	let end = min(coord + radius, input_0_size() - 1);
	let samples = end - start + 1;
	let middle = vec4<u32>(u32(samples.x * samples.y) / 2u);
	var median = vec4<u32>(0u);

	if radius <= 1 {
		for (var candidate_y = start.y; candidate_y <= end.y; candidate_y++) {
			for (var candidate_x = start.x; candidate_x <= end.x; candidate_x++) {
				let candidate = total_order_key(input_0_load(vec2<i32>(candidate_x, candidate_y)));
				var below = vec4<u32>(0u);
				var at_most = vec4<u32>(0u);
				for (var y = start.y; y <= end.y; y++) {
					for (var x = start.x; x <= end.x; x++) {
						let key = total_order_key(input_0_load(vec2<i32>(x, y)));
						below += select(vec4<u32>(0u), vec4<u32>(1u), key < candidate);
						at_most += select(vec4<u32>(0u), vec4<u32>(1u), key <= candidate);
					}
				}
				median = select(median, select(median, candidate, middle < at_most), below <= middle);
			}
		}
		return from_total_order_key(median);
	}

	for (var digit_index = 0u; digit_index < 16u; digit_index++) {
		let shift = 30u - 2u * digit_index;
		var below_one = vec4<u32>(0u);
		var below_two = vec4<u32>(0u);
		var below_three = vec4<u32>(0u);
		for (var y = start.y; y <= end.y; y++) {
			for (var x = start.x; x <= end.x; x++) {
				let key = total_order_key(input_0_load(vec2<i32>(x, y)));
				below_one += select(vec4<u32>(0u), vec4<u32>(1u), key < median + (1u << shift));
				below_two += select(vec4<u32>(0u), vec4<u32>(1u), key < median + (2u << shift));
				below_three += select(vec4<u32>(0u), vec4<u32>(1u), key < median + (3u << shift));
			}
		}
		let digit = select(vec4<u32>(0u), vec4<u32>(1u), below_one <= middle) + select(vec4<u32>(0u), vec4<u32>(1u), below_two <= middle) + select(vec4<u32>(0u), vec4<u32>(1u), below_three <= middle);
		median += digit << vec4<u32>(shift);
	}
	return from_total_order_key(median);
}
