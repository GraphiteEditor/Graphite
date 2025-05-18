use core::f64;
use glam::DVec2;

const DEEPEST_SUBDIVISION_LEVEL_BEFORE_DISCARDING: usize = 8;

/// Fast (O(n) with respect to time and memory) algorithm for generating a maximal set of points using Poisson-disk sampling.
/// Based on the paper:
/// "Poisson Disk Point Sets by Hierarchical Dart Throwing"
/// <https://scholarsarchive.byu.edu/facpub/237/>
pub fn poisson_disk_sample(
	width: f64,
	height: f64,
	diameter: f64,
	point_in_shape_checker: impl Fn(DVec2) -> bool,
	square_edges_intersect_shape_checker: impl Fn(DVec2, f64) -> bool,
	rng: impl FnMut() -> f64,
) -> Vec<DVec2> {
	let mut rng = rng;
	let diameter_squared = diameter.powi(2);

	// Initialize a place to store the generated points within a spatial acceleration structure
	let mut points_grid = AccelerationGrid::new(width, height, diameter);

	// Pick a grid size for the base-level domain that's as large as possible, while also:
	// - Dividing into an integer number of cells across the dartboard domain, to avoid wastefully throwing darts beyond the width and height of the dartboard domain
	// - Being fully covered by the radius around a dart thrown anywhere in its area, where the worst-case is a corner which has a distance of sqrt(2) to the opposite corner
	let greater_dimension = width.max(height);
	let base_level_grid_size = greater_dimension / (greater_dimension * std::f64::consts::SQRT_2 / (diameter / 2.)).ceil();

	// Initialize the problem by including all base-level squares in the active list since they're all part of the yet-to-be-targetted dartboard domain
	let base_level = ActiveListLevel::new_filled(base_level_grid_size, width, height, &point_in_shape_checker, &square_edges_intersect_shape_checker);
	// In the future, if necessary, this could be turned into a fixed-length array with worst-case length `f64::MANTISSA_DIGITS`
	let mut active_list_levels = vec![base_level];

	// Loop until all active squares have been processed, meaning all of the dartboard domain has been checked
	while active_list_levels.iter().any(|active_list| active_list.not_empty()) {
		// Randomly pick a square in the dartboard domain, with probability proportional to its area
		let (active_square_level, active_square_index_in_level) = target_active_square(&active_list_levels, &mut rng);

		// The level contains the list of all active squares at this target square's subdivision depth
		let level = &mut active_list_levels[active_square_level];

		// Take the targetted active square out of the list and get its size
		let active_square = level.take_square(active_square_index_in_level);
		let active_square_size = level.square_size();

		// Skip this target square if it's within range of any current points, since more nearby points could have been added after this square was included in the active list
		if !square_not_covered_by_poisson_points(active_square.top_left_corner(), active_square_size / 2., diameter_squared, &points_grid) {
			continue;
		}

		// Throw a dart by picking a random point within this target square
		let point = {
			let active_top_left_corner = active_square.top_left_corner();
			let x = active_top_left_corner.x + rng() * active_square_size;
			let y = active_top_left_corner.y + rng() * active_square_size;
			(x, y).into()
		};

		// If the dart hit a valid spot, save that point (we're now permanently done with this target square's region)
		if point_not_covered_by_poisson_points(point, diameter_squared, &points_grid) {
			// Silently reject the point if it lies outside the shape
			if active_square.fully_in_shape() || point_in_shape_checker(point) {
				points_grid.insert(point);
			}
		}
		// Otherwise, subdivide this target square and add valid sub-squares back to the active list for later targetting
		else {
			// Discard any targetable domain smaller than this limited number of subdivision levels since it's too small to matter
			let next_level_deeper_level = active_square_level + 1;
			if next_level_deeper_level > DEEPEST_SUBDIVISION_LEVEL_BEFORE_DISCARDING {
				continue;
			}

			// If necessary for the following step, add another layer of depth to store squares at the next subdivision level
			if active_list_levels.len() <= next_level_deeper_level {
				active_list_levels.push(ActiveListLevel::new(active_square_size / 2.))
			}

			// Get the list of active squares at the level of depth beneath this target square's level
			let next_level_deeper = &mut active_list_levels[next_level_deeper_level];

			// Subdivide this target square into four sub-squares; running out of numerical precision will make this terminate at very small scales
			let subdivided_size = active_square_size / 2.;
			let active_top_left_corner = active_square.top_left_corner();
			let subdivided = [
				active_top_left_corner + DVec2::new(0., 0.),
				active_top_left_corner + DVec2::new(subdivided_size, 0.),
				active_top_left_corner + DVec2::new(0., subdivided_size),
				active_top_left_corner + DVec2::new(subdivided_size, subdivided_size),
			];

			// Add the sub-squares which aren't within the radius of a nearby point to the sub-level's active list
			let half_subdivided_size = subdivided_size / 2.;
			let new_sub_squares = subdivided.into_iter().filter_map(|sub_square| {
				// Any sub-squares within the radius of a nearby point are filtered out
				if !square_not_covered_by_poisson_points(sub_square, half_subdivided_size, diameter_squared, &points_grid) {
					return None;
				}

				// Fully inside the shape
				if active_square.fully_in_shape() {
					Some(ActiveSquare::new(sub_square, true))
				}
				// Intersecting the shape's border
				else {
					// The sub-square is fully inside the shape if its top-left corner is inside and its edges don't intersect the shape border
					let sub_square_fully_inside_shape =
						!square_edges_intersect_shape_checker(sub_square, subdivided_size) && point_in_shape_checker(sub_square) && point_in_shape_checker(sub_square + subdivided_size);
					// if !square_edges_intersect_shape_checker(sub_square, subdivided_size) { assert_eq!(point_in_shape_checker(sub_square), point_in_shape_checker(sub_square + subdivided_size)); }
					// Sometimes this fails so it is necessary to also check the bottom right corner.

					Some(ActiveSquare::new(sub_square, sub_square_fully_inside_shape))
				}
			});
			next_level_deeper.add_squares(new_sub_squares);
		}
	}

	points_grid.final_points()
}

/// Randomly pick a square in the dartboard domain, with probability proportional to its area.
/// Returns a tuple with the subdivision level depth and the square index at that depth.
fn target_active_square(active_list_levels: &[ActiveListLevel], rng: &mut impl FnMut() -> f64) -> (usize, usize) {
	let active_squares_total_area: f64 = active_list_levels.iter().map(|active_list| active_list.total_area()).sum();
	let mut index_into_area = rng() * active_squares_total_area;

	for (level, active_list_level) in active_list_levels.iter().enumerate() {
		let subtracted = index_into_area - active_list_level.total_area();
		if subtracted > 0. {
			index_into_area = subtracted;
			continue;
		}

		let active_square_index_in_level = (index_into_area / active_list_levels[level].square_area()).floor() as usize;
		return (level, active_square_index_in_level);
	}

	panic!("index_into_area couldn't be be mapped to a square in any level of the active lists");
}

fn point_not_covered_by_poisson_points(point: DVec2, diameter_squared: f64, points_grid: &AccelerationGrid) -> bool {
	points_grid.nearby_points(point).all(|nearby_point| {
		let x_separation = nearby_point.x - point.x;
		let y_separation = nearby_point.y - point.y;

		x_separation.powi(2) + y_separation.powi(2) > diameter_squared
	})
}

fn square_not_covered_by_poisson_points(point: DVec2, half_square_size: f64, diameter_squared: f64, points_grid: &AccelerationGrid) -> bool {
	let square_center_x = point.x + half_square_size;
	let square_center_y = point.y + half_square_size;

	points_grid.nearby_points(point).all(|nearby_point| {
		let x_distance = (square_center_x - nearby_point.x).abs() + half_square_size;
		let y_distance = (square_center_y - nearby_point.y).abs() + half_square_size;

		x_distance.powi(2) + y_distance.powi(2) > diameter_squared
	})
}

#[inline(always)]
fn cartesian_product<A, B>(a: A, b: B) -> impl Iterator<Item = (A::Item, B::Item)>
where
	A: Iterator + Clone,
	B: Iterator + Clone,
	A::Item: Clone,
	B::Item: Clone,
{
	a.flat_map(move |i| (b.clone().map(move |j| (i.clone(), j))))
}

/// A square (represented by its top left corner position and width/height of `square_size`) that is currently a candidate for targetting by the dart throwing process.
/// The positive sign bit encodes if the square is contained entirely within the masking shape, or negative if it's outside or intersects the shape path.
pub struct ActiveSquare(DVec2);

impl ActiveSquare {
	pub fn new(top_left_corner: DVec2, fully_in_shape: bool) -> Self {
		Self(if fully_in_shape { top_left_corner } else { -top_left_corner })
	}

	pub fn top_left_corner(&self) -> DVec2 {
		self.0.abs()
	}

	pub fn fully_in_shape(&self) -> bool {
		self.0.x.is_sign_positive()
	}
}

pub struct ActiveListLevel {
	/// List of all subdivided squares of the same size that are currently candidates for targetting by the dart throwing process
	active_squares: Vec<ActiveSquare>,
	/// Width and height of the squares in this level of subdivision
	square_size: f64,
	/// Current sum of the area in all active squares in this subdivision level
	total_area: f64,
}

impl ActiveListLevel {
	#[inline(always)]
	pub fn new(square_size: f64) -> Self {
		Self {
			active_squares: Vec::new(),
			square_size,
			total_area: 0.,
		}
	}

	#[inline(always)]
	pub fn new_filled(square_size: f64, width: f64, height: f64, point_in_shape_checker: impl Fn(DVec2) -> bool, square_edges_intersect_shape_checker: impl Fn(DVec2, f64) -> bool) -> Self {
		// These should divide evenly but rounding is to protect against small numerical imprecision errors
		let x_squares = (width / square_size).round() as usize;
		let y_squares = (height / square_size).round() as usize;

		// Populate each square with its top-left corner coordinate
		let active_squares: Vec<_> = cartesian_product(0..x_squares, 0..y_squares)
			.filter_map(|(x, y)| {
				let corner = (x as f64 * square_size, y as f64 * square_size).into();

				let point_in_shape = point_in_shape_checker(corner);
				let square_edges_intersect_shape = square_edges_intersect_shape_checker(corner, square_size);
				let square_not_outside_shape = point_in_shape || square_edges_intersect_shape;
				let square_in_shape = point_in_shape_checker(corner + square_size) && !square_edges_intersect_shape;
				// if !square_edges_intersect_shape { assert_eq!(point_in_shape_checker(corner), point_in_shape_checker(corner + square_size)); }
				// Sometimes this fails so it is necessary to also check the bottom right corner.
				square_not_outside_shape.then_some(ActiveSquare::new(corner, square_in_shape))
			})
			.collect();

		// Sum every square's area to get the total
		let total_area = square_size.powi(2) * active_squares.len() as f64;

		Self {
			active_squares,
			square_size,
			total_area,
		}
	}

	#[must_use]
	#[inline(always)]
	pub fn take_square(&mut self, active_square_index: usize) -> ActiveSquare {
		let targetted_square = self.active_squares.swap_remove(active_square_index);
		self.total_area = self.square_size.powi(2) * self.active_squares.len() as f64;
		targetted_square
	}

	#[inline(always)]
	pub fn add_squares(&mut self, new_squares: impl Iterator<Item = ActiveSquare>) {
		for new_square in new_squares {
			self.active_squares.push(new_square);
		}
		self.total_area = self.square_size.powi(2) * self.active_squares.len() as f64;
	}

	#[inline(always)]
	pub fn square_size(&self) -> f64 {
		self.square_size
	}

	#[inline(always)]
	pub fn square_area(&self) -> f64 {
		self.square_size.powi(2)
	}

	#[inline(always)]
	pub fn total_area(&self) -> f64 {
		self.total_area
	}

	#[inline(always)]
	pub fn not_empty(&self) -> bool {
		!self.active_squares.is_empty()
	}
}

#[derive(Clone, Default)]
pub struct PointsList {
	// The worst-case number of points in a 3x3 grid is 16 (one at each intersection of the four gridlines per axis)
	storage_slots: [DVec2; 16],
	length: usize,
}

impl PointsList {
	#[inline(always)]
	pub fn push(&mut self, point: DVec2) {
		self.storage_slots[self.length] = point;
		self.length += 1;
	}

	#[inline(always)]
	pub fn list_cell_and_neighbors(&self) -> impl Iterator<Item = DVec2> {
		// The negative bit is used to store whether a point belongs to a neighboring cell
		self.storage_slots.into_iter().take(self.length).map(|point| (point.x.abs(), point.y.abs()).into())
	}

	#[inline(always)]
	pub fn list_cell(&self) -> impl Iterator<Item = DVec2> {
		// The negative bit is used to store whether a point belongs to a neighboring cell
		self.storage_slots
			.into_iter()
			.take(self.length)
			.filter(|point| point.x.is_sign_positive() && point.y.is_sign_positive())
	}
}

pub struct AccelerationGrid {
	size: f64,
	dimension_x: usize,
	dimension_y: usize,
	cells: Vec<PointsList>,
}

impl AccelerationGrid {
	#[inline(always)]
	pub fn new(width: f64, height: f64, size: f64) -> Self {
		let dimension_x = (width / size).ceil() as usize + 1;
		let dimension_y = (height / size).ceil() as usize + 1;

		Self {
			size,
			dimension_x,
			dimension_y,
			cells: vec![PointsList::default(); dimension_x * dimension_y],
		}
	}

	#[inline(always)]
	pub fn insert(&mut self, point: DVec2) {
		let x = (point.x / self.size).floor() as usize;
		let y = (point.y / self.size).floor() as usize;

		// Insert this point at this cell and the surrounding cells in a 3x3 patch
		for (x_offset, y_offset) in cartesian_product((-1)..=1, (-1)..=1) {
			// Avoid going negative
			let (x, y) = (x as isize + x_offset, y as isize + y_offset);
			if x < 0 || y < 0 {
				continue;
			}
			// Avoid going beyond the width or height
			let (x, y) = (x as usize, y as usize);
			if x > self.dimension_x - 1 || y > self.dimension_y - 1 {
				continue;
			}

			// Get the cell corresponding to the (x, y) index
			let cell = &mut self.cells[y * self.dimension_x + x];

			// Store the given point in this grid cell, and use the negative bit to indicate if this belongs to a neighboring cell
			cell.push(if x_offset == 0 && y_offset == 0 { point } else { -point });
		}
	}

	#[inline(always)]
	pub fn nearby_points(&self, point: DVec2) -> impl Iterator<Item = DVec2> {
		let x = (point.x / self.size).floor() as usize;
		let y = (point.y / self.size).floor() as usize;

		self.cells[y * self.dimension_x + x].list_cell_and_neighbors()
	}

	#[inline(always)]
	pub fn final_points(&self) -> Vec<DVec2> {
		self.cells.iter().flat_map(|cell| cell.list_cell()).collect()
	}
}
