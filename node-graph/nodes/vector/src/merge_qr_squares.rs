use glam::DVec2;
use graphic_types::Vector;
use std::collections::VecDeque;
use vector_types::subpath;

pub fn merge_qr_squares(qr_code: &qrcodegen::QrCode) -> Vector {
	let mut vector = Vector::default();

	let size = qr_code.size() as usize;

	// 0 = empty
	// 1 = filled, unvisited
	// 2 = filled, current island
	let mut remaining = vec![vec![0u8; size]; size];

	#[allow(clippy::needless_range_loop)]
	for y in 0..size {
		#[allow(clippy::needless_range_loop)]
		for x in 0..size {
			if qr_code.get_module(x as i32, y as i32) {
				remaining[y][x] = 1;
			}
		}
	}

	for y in 0..size {
		for x in 0..size {
			if remaining[y][x] != 1 {
				continue;
			}

			// fill island
			let mut island = Vec::new();
			let mut queue = VecDeque::new();
			queue.push_back((x, y));
			remaining[y][x] = 2;

			while let Some((ix, iy)) = queue.pop_front() {
				island.push((ix, iy));

				for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
					let nx = ix as i32 + dx;
					let ny = iy as i32 + dy;

					if nx >= 0 && nx < size as i32 && ny >= 0 && ny < size as i32 && remaining[ny as usize][nx as usize] == 1 {
						remaining[ny as usize][nx as usize] = 2;
						queue.push_back((nx as usize, ny as usize));
					}
				}
			}

			// boundary detection
			let mut outbound = vec![vec![0u8; size + 1]; size + 1];

			for &(ix, iy) in &island {
				if iy == 0 || remaining[iy - 1][ix] != 2 {
					outbound[iy][ix] |= 1 << 0;
				}
				if ix == size - 1 || remaining[iy][ix + 1] != 2 {
					outbound[iy][ix + 1] |= 1 << 1;
				}
				if iy == size - 1 || remaining[iy + 1][ix] != 2 {
					outbound[iy + 1][ix + 1] |= 1 << 2;
				}
				if ix == 0 || remaining[iy][ix - 1] != 2 {
					outbound[iy + 1][ix] |= 1 << 3;
				}
			}

			// tracing loops
			for vy in 0..=size {
				for vx in 0..=size {
					while outbound[vy][vx] != 0 {
						let mut dir = outbound[vy][vx].trailing_zeros() as usize;
						let start = (vx, vy);
						let mut current = start;
						let mut points = Vec::new();

						loop {
							points.push(DVec2::new(current.0 as f64, current.1 as f64));
							outbound[current.1][current.0] &= !(1 << dir);

							current = match dir {
								0 => (current.0 + 1, current.1),
								1 => (current.0, current.1 + 1),
								2 => (current.0 - 1, current.1),
								3 => (current.0, current.1 - 1),
								_ => unreachable!(),
							};

							if current == start {
								break;
							}
							dir = outbound[current.1][current.0].trailing_zeros() as usize;
						}

						if points.len() > 2 {
							let mut simplified = Vec::new();
							for i in 0..points.len() {
								let prev = points[(i + points.len() - 1) % points.len()];
								let curr = points[i];
								let next = points[(i + 1) % points.len()];
								if (curr - prev).perp_dot(next - curr).abs() > 1e-6 {
									simplified.push(curr);
								}
							}

							if !simplified.is_empty() {
								vector.append_subpath(subpath::Subpath::from_anchors(simplified, true), false);
							}
						}
					}
				}
			}

			// marking island as processed
			for &(ix, iy) in &island {
				remaining[iy][ix] = 0;
			}
		}
	}

	vector
}
