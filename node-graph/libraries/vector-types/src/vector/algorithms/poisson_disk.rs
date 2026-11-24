use crate::vector::misc::{dvec2_to_point, point_to_dvec2};
use glam::DVec2;
use kurbo::{BezPath, CubicBez, ParamCurveExtrema, PathEl, Point, QuadBez};
use robust::{Coord, orient2d};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Squares subdivided this many times without receiving a point are discarded, since what remains uncovered in them is too small to matter.
const DEEPEST_SUBDIVISION_LEVEL: u8 = 12;

/// Squares also stop subdividing once this many darts in them and their ancestors land in uncovered space outside the shape, a sign the shape fills a sliver of them at most.
const MAX_MISSES: u8 = 7;

/// Width of the base-level squares relative to the separation. Being just under 1/sqrt(2), a point anywhere in a square covers all of it, so no square can hold two points.
const BASE_SQUARE_SIZE_PER_SEPARATION: f64 = 0.7;

/// The minimum squared distance between points, in lattice units.
const SEPARATION_SQUARED: f64 = 1. / (BASE_SQUARE_SIZE_PER_SEPARATION * BASE_SQUARE_SIZE_PER_SEPARATION);

/// How closely, in base squares, the boundary's straight-line approximation follows its curves (a thousandth of the separation), when testing which darts land inside.
const FLATTENING_TOLERANCE: f64 = 1e-3 / BASE_SQUARE_SIZE_PER_SEPARATION;

/// The gap, in base squares, beyond which parts of the shape can't affect one another's points, being wider than the separation.
const GROUP_GAP: f64 = 2.;

/// The most subpaths that splitting a shape into separate parts may visit, after which the parts left are kept whole (merely larger than necessary).
const MAX_GROUPING_VISITS: usize = 1 << 24;

/// How far, in base squares, an edge is also listed in the squares beside it, so rounding can't leave it out of a square it touches.
const EDGE_LISTING_PADDING: f64 = 1e-3;

/// Darts land on a grid of 2^24 steps across each base square, so a point's offset within its base square is exact as an `f32` and distances between nearby points are exact as `f64`s.
/// Squares at the deepest subdivision level still leave thousands of steps across them.
const DART_POSITION_BITS: u32 = 24;
const DART_POSITIONS_PER_BASE_SQUARE: f64 = (1 << DART_POSITION_BITS) as f64;

// Limits on the work that sampling may take, counted across all the shapes sampled together, which keep extreme inputs from exhausting the time and memory available

/// The most points the shapes may need, judging by the area of the base squares they reach.
const MAX_POINTS: usize = 1_000_000;

/// The most base squares the grids over the shapes' separate parts may span in total.
const MAX_GRID_SQUARES: f64 = (1_u64 << 23) as f64;

/// The most straight edges the shapes' boundaries may be approximated with.
const MAX_EDGES: usize = 1 << 22;

/// The most base squares the shapes' edges may be listed in, all together.
const MAX_EDGE_LISTINGS: usize = 1 << 24;

/// The most darts the shapes may need thrown, several times what filling the base squares of [`MAX_POINTS`] takes.
const MAX_DARTS: usize = 1 << 25;

/// The most edges the darts' insideness tests may examine, all together.
const MAX_EDGE_TESTS: usize = 1 << 28;

/// The most squares that may wait for their darts at once.
const MAX_WAITING_SQUARES: usize = 1 << 22;

/// The farthest a shape may extend from the origin, in base squares, where positions still resolve to about a millionth of a base square.
const MAX_LATTICE_COORDINATE: f64 = (1_u64 << 32) as f64;

/// The failure of [`poisson_disk_points`].
#[derive(Debug, thiserror::Error)]
pub enum PoissonDiskError {
	#[error("The separation is too small for this shape: it could need more than {limit} points")]
	SeparationTooSmall { limit: usize },
	#[error("The shape spans too large an area to place points with this small of a separation")]
	ExtentTooLarge,
	#[error("The shape has too much fine detail to place points with this small of a separation")]
	TooMuchDetail,
	#[error("The shape is too far from its origin to place points with this small of a separation")]
	TooFarFromOrigin,
}

/// Randomly places points across the area filled by these subpaths (each closed, and combined by the nonzero fill rule), at least `separation` apart from one another.
/// Conceptually, this works by "throwing darts" and keeping each only if:
/// - It's inside the shape.
/// - It's not closer than `separation` to any point from a previously kept dart.
///
/// This repeats until kept darts fill all possible areas between one another.
///
/// The darts are fixed in the geometry's coordinate space by the `seed`, so reshaping part of the boundary only changes the points near that part.
/// Each point comes with a hash that identifies it across such changes. Subpaths with non-finite coordinates are skipped.
///
/// This uses hierarchical dart throwing, based on the paper "Poisson Disk Point Sets by Hierarchical Dart Throwing" <https://scholarsarchive.byu.edu/facpub/237/>.
/// Each square of a quadtree over the dartboard owns a dart, with its position and arrival time hashed from the `seed` and the square's address.
/// Subdividing only the squares that could still take a dart makes this O(n log n) for n points, unless the boundary has detail far finer than the separation.
///
/// Returns each point, in rows from the top-left, with its hash.
pub fn poisson_disk_points(bezpaths: &[BezPath], separation: f64, seed: u64) -> Result<Vec<(DVec2, u64)>, PoissonDiskError> {
	PoissonDiskSampler::default().points(bezpaths, separation, seed)
}

/// Places points like [`poisson_disk_points`] in each of many shapes, holding them all together to its limits on work.
#[derive(Default)]
pub struct PoissonDiskSampler {
	workload: Workload,
	/// Kept from shape to shape so its buckets keep their allocations.
	waiting: ArrivalQueue,
}

impl PoissonDiskSampler {
	/// The points [`poisson_disk_points`] places in these subpaths, with the work counted on top of what earlier calls spent.
	pub fn points(&mut self, bezpaths: &[BezPath], separation: f64, seed: u64) -> Result<Vec<(DVec2, u64)>, PoissonDiskError> {
		if !separation.is_finite() || separation <= 0. {
			return Ok(Vec::new());
		}
		self.sample_shape(bezpaths, DartLattice::new(separation, seed))
	}

	fn sample_shape(&mut self, bezpaths: &[BezPath], lattice: DartLattice) -> Result<Vec<(DVec2, u64)>, PoissonDiskError> {
		// Each subpath with finite coordinates, moved into lattice units, with the bounds of its curves
		let subpaths: Vec<_> = bezpaths
			.iter()
			.flat_map(|bezpath| bezpath.elements().chunk_by(|_, next| !matches!(next, PathEl::MoveTo(_))))
			.filter(|subpath| subpath.iter().flat_map(control_points).all(|point| point.is_finite()))
			.filter_map(|subpath| {
				let subpath: Vec<_> = subpath.iter().map(|&element| lattice.element_to_lattice(element)).collect();

				// A subpath within the flattening tolerance of a line, like an open straight path, encloses too little area to sample
				let control_points: Vec<_> = subpath.iter().flat_map(control_points).map(point_to_dvec2).collect();
				if nearly_collinear(&control_points) {
					return None;
				}

				let bounds = curve_bounds(&subpath);
				Some((subpath, bounds))
			})
			.collect();
		if subpaths.is_empty() {
			return Ok(Vec::new());
		}
		if subpaths.iter().any(|(_, (min, max))| min.abs().max(max.abs()).max_element() > MAX_LATTICE_COORDINATE) {
			return Err(PoissonDiskError::TooFarFromOrigin);
		}

		// Parts of the shape too far apart to affect one another get grids of their own, so the space between them costs nothing
		let bounds: Vec<_> = subpaths.iter().map(|&(_, bounds)| bounds).collect();
		let groups = separate_groups(&bounds);
		for group in &groups {
			let (min, max) = group
				.iter()
				.fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(min, max), &index| (min.min(bounds[index].0), max.max(bounds[index].1)));
			self.workload.grid_squares += (max.x.floor() - min.x.floor() + 1.) * (max.y.floor() - min.y.floor() + 1.);
		}
		if self.workload.grid_squares > MAX_GRID_SQUARES {
			return Err(PoissonDiskError::ExtentTooLarge);
		}

		// Every group's grid is laid out first, so a shape needing too many points is refused before any darts are thrown
		let mut parts = Vec::new();
		for group in groups {
			let edges = lattice_edges(group.iter().map(|&index| subpaths[index].0.as_slice()), &mut self.workload)?;
			if !edges.is_empty() {
				parts.push(Part::new(edges, &mut self.workload)?);
			}
		}

		let mut points = Vec::new();
		for part in parts {
			part.throw_darts(&lattice, &mut self.workload, &mut self.waiting, &mut points)?;
		}

		// Rows of base squares from the top-left, across all the groups
		points.sort_unstable_by_key(|&((x, y), ..)| (y, x));
		Ok(points.into_iter().map(|(_, point, hash)| (lattice.to_geometry(point), hash)).collect())
	}
}

/// The work spent so far by everything sampled together, which is held to the limits.
#[derive(Default)]
struct Workload {
	grid_squares: f64,
	edges: usize,
	edge_listings: usize,
	active_base_squares: usize,
	darts: usize,
	edge_tests: usize,
}

/// One group of the shape's edges, in lattice units, with its grid laid out.
struct Part {
	shape: ShapeGrid,
	/// How many of the grid's base squares aren't outside the shape.
	active_base_squares: usize,
}

impl Part {
	fn new(edges: Vec<(DVec2, DVec2)>, workload: &mut Workload) -> Result<Self, PoissonDiskError> {
		let (min, max) = edges
			.iter()
			.fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(min, max), &(start, end)| (min.min(start).min(end), max.max(start).max(end)));
		let shape = ShapeGrid::new(edges, min, max, workload)?;

		// Refuse a shape that could need too many points, judging by the area of the base squares it reaches
		let active_base_squares = shape.active_base_squares().count();
		workload.active_base_squares += active_base_squares;
		if workload.active_base_squares as f64 > MAX_POINTS as f64 * SEPARATION_SQUARED {
			return Err(PoissonDiskError::SeparationTooSmall { limit: MAX_POINTS });
		}

		Ok(Self { shape, active_base_squares })
	}

	/// Throws the darts, adding the points it accepts to `points` with the addresses of their base squares.
	fn throw_darts(self, lattice: &DartLattice, workload: &mut Workload, waiting: &mut ArrivalQueue, points: &mut Vec<((i64, i64), DVec2, u64)>) -> Result<(), PoissonDiskError> {
		let Self { shape, active_base_squares } = self;
		let mut accepted = AcceptedPoints::new(&shape);
		let column_bits = usize::BITS - (shape.columns - 1).leading_zeros();

		// Every base square not outside the shape becomes active, its dart arriving after an exponentially distributed wait
		waiting.clear(active_base_squares);
		for (column, row, inside) in shape.active_base_squares() {
			let square = Square {
				column,
				row,
				level: 0,
				sub_column: 0,
				sub_row: 0,
				inside,
				misses: 0,
			};
			let (x, y) = square.address(&shape);
			waiting.push(ActiveSquare::new(wait(lattice.square_hash(0, x, y)), square, column_bits));
		}

		// Take each dart in order of arrival
		while let Some(active) = waiting.pop() {
			// Refuse a shape with so much boundary enclosing so little area that the darts could go on missing it at length
			workload.darts += 1;
			if workload.darts > MAX_DARTS || workload.edge_tests > MAX_EDGE_TESTS || waiting.len() > MAX_WAITING_SQUARES {
				return Err(PoissonDiskError::TooMuchDetail);
			}

			// Skip the square if a point accepted since it became active now covers all of it, as any point in its base square does
			let square = active.square(column_bits);
			let (column, row) = (square.column, square.row);
			if accepted.holds_point(column, row) {
				continue;
			}
			let (square_min, square_size) = square.bounds_in_base_square();
			let nearby = accepted.nearby(column, row, square_min, square_size);
			if nearby.covers_square(square_min, square_size) {
				continue;
			}

			// Accept the dart if it's far enough from every accepted point and inside the shape
			let (x, y) = square.address(&shape);
			let dart = square_min + dart_within(lattice.square_hash(square.level, x, y), square.level);
			let in_open_space = !nearby.covers_point(dart);
			if in_open_space && (square.inside || shape.winding(column, row, dart, &mut workload.edge_tests) != 0) {
				accepted.insert(dart, column, row);
				continue;
			}

			// Otherwise, leave whatever is still uncovered to the four sub-squares, each waiting for a dart of its own
			let misses = square.misses + in_open_space as u8;
			if square.level == DEEPEST_SUBDIVISION_LEVEL || misses == MAX_MISSES {
				continue;
			}
			let level = square.level + 1;
			let size = square_size / 2.;
			let mut column_hashes = [None; 2];
			for (column_offset, row_offset) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
				let min = square_min + DVec2::new(column_offset as f64, row_offset as f64) * size;
				if nearby.covers_square(min, size) {
					continue;
				}

				let inside = square.inside
					|| match shape.coverage(column, row, min, size, &mut workload.edge_tests) {
						Coverage::Outside => continue,
						Coverage::Inside => true,
						Coverage::Straddling => false,
					};

				// The two sub-squares in each column share the first part of their hashes
				let column_hash = *column_hashes[column_offset].get_or_insert_with(|| lattice.column_hash(level, x * 2 + column_offset as i64));
				let hash = square_hash_in_column(column_hash, y * 2 + row_offset);

				// Darts arrive at a rate proportional to a square's area, so each level's mean wait is four times the last
				let wait_scale = (1_u64 << (2 * level)) as f64;
				let sub_square = Square {
					level,
					sub_column: square.sub_column * 2 + column_offset as u32,
					sub_row: square.sub_row * 2 + row_offset as u32,
					inside,
					misses,
					..square
				};
				waiting.push(ActiveSquare::new(active.time + wait_scale * wait(hash), sub_square, column_bits));
			}
		}

		points.extend(accepted.into_points(lattice));
		Ok(())
	}
}

/// Splits the subpaths, given by the indices of their `bounds`, into groups spaced more than [`GROUP_GAP`] apart, which can't affect one another's points.
/// Gaps are found by sorting along one axis and then the other, repeating within each part until neither splits it further.
fn separate_groups(bounds: &[(DVec2, DVec2)]) -> Vec<Vec<usize>> {
	let mut groups = Vec::new();
	let mut unsplit = vec![(0..bounds.len()).collect::<Vec<_>>()];
	let mut visits = 0;

	while let Some(mut group) = unsplit.pop() {
		// Arrangements that only ever split off one subpath at a time would take quadratic time, so past a point the rest stay whole
		visits += group.len();
		if visits > MAX_GROUPING_VISITS {
			groups.push(group);
			groups.append(&mut unsplit);
			break;
		}

		match [0, 1].into_iter().map(|axis| split_along(&mut group, axis, bounds)).find(|parts| parts.len() > 1) {
			Some(parts) => unsplit.extend(parts),
			None => groups.push(group),
		}
	}

	groups
}

/// Divides the group wherever a sweep along the axis finds a gap wider than [`GROUP_GAP`] between the bounds it has passed and those ahead.
fn split_along(group: &mut [usize], axis: usize, bounds: &[(DVec2, DVec2)]) -> Vec<Vec<usize>> {
	group.sort_unstable_by(|&a, &b| bounds[a].0[axis].total_cmp(&bounds[b].0[axis]));

	let mut parts = Vec::new();
	let mut part_start = 0;
	let mut reach = f64::NEG_INFINITY;
	for (i, &index) in group.iter().enumerate() {
		let (min, max) = bounds[index];
		if i > 0 && min[axis] > reach + GROUP_GAP {
			parts.push(group[part_start..i].to_vec());
			part_start = i;
		}
		reach = reach.max(max[axis]);
	}
	parts.push(group[part_start..].to_vec());

	parts
}

/// Approximates each subpath's boundary, in lattice units, with straight edges, closing it back to its start.
fn lattice_edges<'a>(subpaths: impl Iterator<Item = &'a [PathEl]>, workload: &mut Workload) -> Result<Vec<(DVec2, DVec2)>, PoissonDiskError> {
	let mut edges = Vec::new();

	// Storing stops once the edges reach the limit, since a single subpath of huge curves could otherwise flatten into more than memory holds
	let room = MAX_EDGES.saturating_sub(workload.edges);
	let mut overflowed = false;
	let mut push = |edges: &mut Vec<_>, edge| {
		if edges.len() < room {
			edges.push(edge);
		} else {
			overflowed = true;
		}
	};

	for subpath in subpaths {
		let mut start = None;
		let mut previous = DVec2::ZERO;
		kurbo::flatten(subpath.iter().copied(), FLATTENING_TOLERANCE, |element| {
			let point = match element {
				PathEl::MoveTo(point) | PathEl::LineTo(point) => point_to_dvec2(point),
				PathEl::ClosePath => start.unwrap_or(previous),
				PathEl::QuadTo(..) | PathEl::CurveTo(..) => return,
			};

			if matches!(element, PathEl::MoveTo(_)) || start.is_none() {
				start = Some(point);
			} else if point != previous {
				push(&mut edges, (previous, point));
			}
			previous = point;
		});
		if let Some(start) = start
			&& start != previous
		{
			push(&mut edges, (previous, start));
		}
	}

	if overflowed {
		return Err(PoissonDiskError::TooMuchDetail);
	}
	workload.edges += edges.len();
	Ok(edges)
}

/// Whether every point lies within the flattening tolerance of a single line, checked along the line from the first point to the farthest.
fn nearly_collinear(points: &[DVec2]) -> bool {
	let Some(&start) = points.first() else { return true };
	let distance_from_start = |point: &&DVec2| point.distance_squared(start);
	let farthest = points.iter().max_by(|a, b| distance_from_start(a).total_cmp(&distance_from_start(b))).copied().unwrap_or(start);
	let direction = (farthest - start).normalize_or_zero();

	points.iter().all(|&point| (point - start).perp_dot(direction).abs() <= FLATTENING_TOLERANCE)
}

/// The bounds of a subpath's curves, which can be tighter than those of its control points.
fn curve_bounds(subpath: &[PathEl]) -> (DVec2, DVec2) {
	let mut bounds = (DVec2::INFINITY, DVec2::NEG_INFINITY);
	let mut include = |rect: kurbo::Rect| bounds = (bounds.0.min(DVec2::new(rect.x0, rect.y0)), bounds.1.max(DVec2::new(rect.x1, rect.y1)));

	let mut current = None;
	for &element in subpath {
		// A curve without a point to start from (in a malformed path) is bounded by its control points instead
		let start = current.unwrap_or_else(|| control_points(&element).next().unwrap_or_default());
		match element {
			PathEl::MoveTo(point) | PathEl::LineTo(point) => include(kurbo::Rect::from_points(point, point)),
			PathEl::QuadTo(p1, p2) => include(QuadBez::new(start, p1, p2).bounding_box()),
			PathEl::CurveTo(p1, p2, p3) => include(CubicBez::new(start, p1, p2, p3).bounding_box()),
			PathEl::ClosePath => {}
		}
		current = element.end_point().or(current);
	}

	bounds
}

/// The points an element adds to its path, which together bound the path's curves.
fn control_points(element: &PathEl) -> impl Iterator<Item = Point> {
	let (points, count) = match *element {
		PathEl::MoveTo(point) | PathEl::LineTo(point) => ([point; 3], 1),
		PathEl::QuadTo(p1, p2) => ([p1, p2, p2], 2),
		PathEl::CurveTo(p1, p2, p3) => ([p1, p2, p3], 3),
		PathEl::ClosePath => ([Point::ZERO; 3], 0),
	};
	points.into_iter().take(count)
}

/// The dartboard, a lattice of base squares one unit wide, positioned in the geometry's coordinate space by the separation and seed alone.
struct DartLattice {
	origin: DVec2,
	square_size: f64,
	seed_hash: u64,
	/// The first part of the hash of every square at each subdivision level.
	level_hashes: [u64; DEEPEST_SUBDIVISION_LEVEL as usize + 1],
}

impl DartLattice {
	fn new(separation: f64, seed: u64) -> Self {
		let square_size = separation * BASE_SQUARE_SIZE_PER_SEPARATION;

		// A fractional offset keeps geometry at round-numbered coordinates from systematically lying along the lattice lines
		let offset_hash = mix(seed ^ 0x6C61_7474_6963_6521);
		let origin = DVec2::new(unit_interval(mix(offset_hash ^ 1)), unit_interval(mix(offset_hash ^ 2))) * square_size;

		let seed_hash = mix(seed);
		Self {
			origin,
			square_size,
			seed_hash,
			level_hashes: std::array::from_fn(|level| mix(seed_hash ^ level as u64)),
		}
	}

	fn to_lattice(&self, point: DVec2) -> DVec2 {
		(point - self.origin) / self.square_size
	}

	fn to_geometry(&self, point: DVec2) -> DVec2 {
		self.origin + point * self.square_size
	}

	fn element_to_lattice(&self, element: PathEl) -> PathEl {
		let to_lattice = |point: Point| dvec2_to_point(self.to_lattice(point_to_dvec2(point)));
		match element {
			PathEl::MoveTo(point) => PathEl::MoveTo(to_lattice(point)),
			PathEl::LineTo(point) => PathEl::LineTo(to_lattice(point)),
			PathEl::QuadTo(p1, p2) => PathEl::QuadTo(to_lattice(p1), to_lattice(p2)),
			PathEl::CurveTo(p1, p2, p3) => PathEl::CurveTo(to_lattice(p1), to_lattice(p2), to_lattice(p3)),
			PathEl::ClosePath => PathEl::ClosePath,
		}
	}

	/// The hash of the square at this address, from which its dart's arrival and position derive, depending on nothing but the address and seed.
	#[inline]
	fn square_hash(&self, level: u8, x: i64, y: i64) -> u64 {
		square_hash_in_column(self.column_hash(level, x), y)
	}

	/// The part of [`Self::square_hash`] shared by the squares in one column of a subdivision level.
	#[inline]
	fn column_hash(&self, level: u8, x: i64) -> u64 {
		mix(self.level_hashes[level as usize] ^ x as u64)
	}

	/// The hash identifying a point by the dart it came from, which its base square and offset within it pin down.
	fn point_hash(&self, base_x: i64, base_y: i64, [offset_x, offset_y]: [f32; 2]) -> u64 {
		let offset = ((offset_x.to_bits() as u64) << 32) | offset_y.to_bits() as u64;
		[base_x as u64, base_y as u64, offset].into_iter().fold(self.seed_hash, |hash, value| mix(hash ^ value))
	}
}

/// The rest of [`DartLattice::square_hash`] after the part shared by the square's column.
#[inline]
fn square_hash_in_column(column_hash: u64, y: i64) -> u64 {
	mix(column_hash ^ y as u64)
}

/// How long after its square becomes active a dart arrives, exponentially distributed with a mean of one (and never negative zero, which would sort after every other time by its bits).
#[inline]
fn wait(square_hash: u64) -> f64 {
	0. - (1. - unit_interval(mix(square_hash ^ 1))).ln()
}

/// Where a dart lands from its square's top-left corner, in base squares, at one of the dart grid's steps within the square.
#[inline]
fn dart_within(square_hash: u64, level: u8) -> DVec2 {
	let step = |hash: u64| (hash >> (64 - DART_POSITION_BITS + level as u32)) as f64 / DART_POSITIONS_PER_BASE_SQUARE;
	DVec2::new(step(mix(square_hash ^ 2)), step(mix(square_hash ^ 3)))
}

/// The SplitMix64 finalizer, which spreads every input bit across the result.
#[inline]
fn mix(mut value: u64) -> u64 {
	value ^= value >> 30;
	value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
	value ^= value >> 27;
	value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
	value ^ (value >> 31)
}

/// A hash's top 53 bits as a fraction from 0 up to (but excluding) 1, which an f64 holds exactly.
#[inline]
fn unit_interval(hash: u64) -> f64 {
	(hash >> 11) as f64 / (1_u64 << 53) as f64
}

/// A square of the quadtree over a part's grid, by its base square and its place within that base square.
#[derive(Clone, Copy)]
struct Square {
	/// The base square's column and row in the part's grid.
	column: usize,
	row: usize,
	level: u8,
	/// The square's column and row among the 2^level by 2^level squares that its level divides the base square into.
	sub_column: u32,
	sub_row: u32,
	/// Whether the square lies entirely inside the shape, so its darts need no insideness test.
	inside: bool,
	/// How many darts of the square's ancestors landed in uncovered space outside the shape.
	misses: u8,
}

impl Square {
	/// The square's top-left corner, from that of its base square, and its width, in base squares.
	#[inline]
	fn bounds_in_base_square(&self) -> (DVec2, f64) {
		// Exactly 2^-level, built from its exponent bits rather than by a slow division
		let size = f64::from_bits((1023 - self.level as u64) << 52);
		(DVec2::new(self.sub_column as f64, self.sub_row as f64) * size, size)
	}

	/// The square's column and row among all of the lattice's squares at its level.
	#[inline]
	fn address(&self, shape: &ShapeGrid) -> (i64, i64) {
		let (base_x, base_y) = (shape.min_x + self.column as i64, shape.min_y + self.row as i64);
		((base_x << self.level) + self.sub_column as i64, (base_y << self.level) + self.sub_row as i64)
	}
}

// The packing of an `ActiveSquare` leaves 12 bits for a square's place along each axis within its base square, 4 for its level, and 3 for its misses
const _: () = assert!(DEEPEST_SUBDIVISION_LEVEL <= 12 && MAX_MISSES <= 8);

/// A [`Square`] waiting for its dart to arrive at `time`, packed into 16 bytes since moving squares through the queue is much of the work.
#[derive(Clone, Copy)]
struct ActiveSquare {
	time: f64,
	/// From the highest bits: the base square's row and column (32 bits, the column taking as many of the lowest as the grid's columns need), the level (4 bits), the column and row within the base square (12 bits each), whether it's inside (1 bit), and the misses (3 bits).
	packed: u64,
}

impl ActiveSquare {
	/// Packs the square, whose part's grid needs `column_bits` bits to number its columns and fewer than 32 more for its rows.
	#[inline]
	fn new(time: f64, square: Square, column_bits: u32) -> Self {
		let base = ((square.row as u64) << column_bits) | square.column as u64;
		let within_base = ((square.level as u64) << 28) | ((square.sub_column as u64) << 16) | ((square.sub_row as u64) << 4);
		let packed = (base << 32) | within_base | ((square.inside as u64) << 3) | square.misses as u64;
		Self { time, packed }
	}

	#[inline]
	fn square(&self, column_bits: u32) -> Square {
		let base = self.packed >> 32;
		Square {
			column: (base & ((1 << column_bits) - 1)) as usize,
			row: (base >> column_bits) as usize,
			level: (self.packed >> 28) as u8 & 0xF,
			sub_column: (self.packed >> 16) as u32 & 0xFFF,
			sub_row: (self.packed >> 4) as u32 & 0xFFF,
			inside: self.packed & 0b1000 != 0,
			misses: self.packed as u8 & 0b111,
		}
	}
}

/// Bits of a time per digit, by which [`ArrivalQueue`] files the squares.
const DIGIT_BITS: u32 = 4;
const DIGIT_VALUES: usize = 1 << DIGIT_BITS;

/// One bucket for times equal to the last given out, then one for each value of each digit.
const BUCKETS: usize = 1 + (u64::BITS / DIGIT_BITS) as usize * DIGIT_VALUES;

/// How many squares each block of a bucket's storage holds.
const CHUNK_SQUARES: usize = 16;

/// Marks the end of a list of chunks.
const NO_CHUNK: u32 = u32::MAX;

/// A block of a bucket's squares, linked to the bucket's next block (or once freed, to the next free one).
#[derive(Clone, Copy)]
struct Chunk {
	squares: [ActiveSquare; CHUNK_SQUARES],
	len: u32,
	next: u32,
}

/// The most base squares a part may start with for its squares to wait in a binary heap, which is quicker than the radix heap up to about this size.
const FEW_BASE_SQUARES: usize = 1 << 15;

/// The squares waiting for their darts, given out in order of arrival (and among equal times, in a fixed order of their packing).
///
/// A radix heap: no square arrives before the last one given out, so each is filed by the highest hexadecimal digit where its time's bits differ from that last time's, and by its value there.
/// Giving one out only redistributes the lowest nonempty bucket, so a square moves at most once per digit, with sequential memory access.
/// The buckets store their squares in chunks from one shared pool, so filling many buckets doesn't allocate for each.
struct ArrivalQueue {
	/// The bits of the last time given out, which preserve the order of non-negative times.
	last: u64,
	/// Each bucket's list of chunks, starting from the one being filled.
	buckets: [u32; BUCKETS],
	/// One bit per bucket that holds squares.
	occupied: [u64; BUCKETS.div_ceil(64)],
	/// Every bucket's chunks, kept from shape to shape.
	chunks: Vec<Chunk>,
	/// The list of chunks free for reuse.
	free: u32,
	/// Where the squares of a part with few base squares wait instead, as their time's bits and their packing, in the same order.
	few: BinaryHeap<Reverse<(u64, u64)>>,
	holds_few: bool,
	len: usize,
}

impl Default for ArrivalQueue {
	fn default() -> Self {
		Self {
			last: 0,
			buckets: [NO_CHUNK; BUCKETS],
			occupied: [0; BUCKETS.div_ceil(64)],
			chunks: Vec::new(),
			free: NO_CHUNK,
			few: BinaryHeap::new(),
			holds_few: false,
			len: 0,
		}
	}
}

impl ArrivalQueue {
	#[inline]
	fn len(&self) -> usize {
		self.len
	}

	/// Empties the queue, so it can take squares arriving from time zero again, starting with this many base squares.
	fn clear(&mut self, base_squares: usize) {
		self.buckets.fill(NO_CHUNK);
		self.occupied.fill(0);
		self.chunks.clear();
		self.free = NO_CHUNK;
		self.few.clear();
		self.last = 0;
		self.len = 0;

		self.holds_few = base_squares <= FEW_BASE_SQUARES;
		if !self.holds_few {
			self.chunks.reserve(base_squares.div_ceil(CHUNK_SQUARES) + BUCKETS / 4);
		}
	}

	#[inline]
	fn push(&mut self, square: ActiveSquare) {
		self.len += 1;
		if self.holds_few {
			self.few.push(Reverse((square.time.to_bits(), square.packed)));
		} else {
			self.file(square);
		}
	}

	/// Adds the square to the bucket for its time.
	#[inline]
	fn file(&mut self, square: ActiveSquare) {
		let time = square.time.to_bits();
		let difference = time ^ self.last;
		let bucket = if difference == 0 {
			0
		} else {
			let digit = (u64::BITS - 1 - difference.leading_zeros()) / DIGIT_BITS;
			1 + digit as usize * DIGIT_VALUES + (time >> (digit * DIGIT_BITS)) as usize % DIGIT_VALUES
		};

		// Start the bucket a new chunk when the one it's filling is full
		let head = self.buckets[bucket];
		let chunk = if head != NO_CHUNK && (self.chunks[head as usize].len as usize) < CHUNK_SQUARES {
			head
		} else {
			let chunk = self.take_chunk(head);
			self.buckets[bucket] = chunk;
			chunk
		};

		let chunk = &mut self.chunks[chunk as usize];
		chunk.squares[chunk.len as usize] = square;
		chunk.len += 1;
		self.occupied[bucket / 64] |= 1 << (bucket % 64);
	}

	/// An empty chunk, from the free ones if there are any, linked to `next`.
	fn take_chunk(&mut self, next: u32) -> u32 {
		if self.free == NO_CHUNK {
			let squares = [ActiveSquare { time: 0., packed: 0 }; CHUNK_SQUARES];
			self.chunks.push(Chunk { squares, len: 0, next });
			return (self.chunks.len() - 1) as u32;
		}

		let chunk = self.free;
		self.free = self.chunks[chunk as usize].next;
		self.chunks[chunk as usize].len = 0;
		self.chunks[chunk as usize].next = next;
		chunk
	}

	fn free_chunk(&mut self, chunk: u32) {
		self.chunks[chunk as usize].next = self.free;
		self.free = chunk;
	}

	#[inline]
	fn pop(&mut self) -> Option<ActiveSquare> {
		if self.holds_few {
			let Reverse((time, packed)) = self.few.pop()?;
			self.len -= 1;
			return Some(ActiveSquare { time: f64::from_bits(time), packed });
		}

		// Once the earliest time is used up, the lowest nonempty bucket holds the next, and its squares spread into the buckets below it
		if self.occupied[0] & 1 == 0 {
			let (word, &bits) = self.occupied.iter().enumerate().find(|(_, bits)| **bits != 0)?;
			let lowest = word * 64 + bits.trailing_zeros() as usize;
			self.occupied[word] &= !(1 << (lowest % 64));
			let first_chunk = std::mem::replace(&mut self.buckets[lowest], NO_CHUNK);

			let mut chunk = first_chunk;
			let mut earliest = u64::MAX;
			while chunk != NO_CHUNK {
				let Chunk { squares, len, next } = &self.chunks[chunk as usize];
				earliest = squares[..*len as usize].iter().fold(earliest, |earliest, square| earliest.min(square.time.to_bits()));
				chunk = *next;
			}
			self.last = earliest;

			// None of the squares return to this bucket, so its chunks are freed as they're emptied
			let mut chunk = first_chunk;
			while chunk != NO_CHUNK {
				for index in 0..self.chunks[chunk as usize].len as usize {
					let square = self.chunks[chunk as usize].squares[index];
					self.file(square);
				}
				let next = self.chunks[chunk as usize].next;
				self.free_chunk(chunk);
				chunk = next;
			}
		}

		// The squares of the earliest time, nearly always just one, are taken in the order of their packing
		let first_chunk = self.buckets[0];
		let (mut next_chunk, mut next_index) = (first_chunk, 0);
		let mut chunk = first_chunk;
		while chunk != NO_CHUNK {
			let Chunk { squares, len, next } = &self.chunks[chunk as usize];
			for (index, square) in squares[..*len as usize].iter().enumerate() {
				if square.packed < self.chunks[next_chunk as usize].squares[next_index].packed {
					(next_chunk, next_index) = (chunk, index);
				}
			}
			chunk = *next;
		}
		let square = self.chunks[next_chunk as usize].squares[next_index];

		// Fill its place with the last square of the chunk being filled, freeing that chunk if it empties
		let filling = &mut self.chunks[first_chunk as usize];
		filling.len -= 1;
		let moved = filling.squares[filling.len as usize];
		let filling_emptied = filling.len == 0;
		let filling_next = filling.next;
		self.chunks[next_chunk as usize].squares[next_index] = moved;
		if filling_emptied {
			self.buckets[0] = filling_next;
			self.free_chunk(first_chunk);
			if filling_next == NO_CHUNK {
				self.occupied[0] &= !1;
			}
		}

		self.len -= 1;
		Some(square)
	}
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Coverage {
	Inside,
	Outside,
	Straddling,
}

#[inline]
fn coordinate(point: DVec2) -> Coord<f64> {
	Coord { x: point.x, y: point.y }
}

/// Exactly what [`orient2d`] returns, with its quick test for a clearly nonzero result inlined and the rest left to it.
#[inline]
fn orient(a: Coord<f64>, b: Coord<f64>, c: Coord<f64>) -> f64 {
	// The error bound for the rounded determinant, as `robust` computes it
	const ERROR_BOUND_PER_MAGNITUDE: f64 = 3.330_669_062_177_372_2e-16;

	let left = (a.x - c.x) * (b.y - c.y);
	let right = (a.y - c.y) * (b.x - c.x);
	let determinant = left - right;
	let error_bound = ERROR_BOUND_PER_MAGNITUDE * (left + right).abs();

	if determinant >= error_bound || -determinant >= error_bound {
		determinant
	} else {
		orient2d(a, b, c)
	}
}

/// Each row of base squares an edge touches within a grid, with the first and last column it touches there.
fn touched_spans(&(start, end): &(DVec2, DVec2), min_x: i64, min_y: i64, columns: usize, rows: usize) -> impl Iterator<Item = (i64, i64, i64)> {
	let (low, high) = (start.y.min(end.y) - EDGE_LISTING_PADDING, start.y.max(end.y) + EDGE_LISTING_PADDING);
	let first_row = (low.floor() as i64).max(min_y);
	let last_row = (high.floor() as i64).min(min_y + rows as i64 - 1);
	let (edge_left, edge_right) = (start.x.min(end.x), start.x.max(end.x));

	(first_row..=last_row).map(move |row| {
		// The span of the edge across this row
		let band_low = (row as f64 - EDGE_LISTING_PADDING).max(low);
		let band_high = (row as f64 + 1. + EDGE_LISTING_PADDING).min(high);
		let (left, right) = if start.y == end.y {
			(edge_left, edge_right)
		} else {
			let x_at = |y: f64| start.x + (y - start.y) * (end.x - start.x) / (end.y - start.y);
			let (x_at_low, x_at_high) = (x_at(band_low), x_at(band_high));
			(x_at_low.min(x_at_high).max(edge_left), x_at_low.max(x_at_high).min(edge_right))
		};

		let first_column = ((left - EDGE_LISTING_PADDING).floor() as i64).max(min_x);
		let last_column = ((right + EDGE_LISTING_PADDING).floor() as i64).min(min_x + columns as i64 - 1);
		(row, first_column, last_column)
	})
}

/// The shape's boundary edges in lattice units, listed by each base square they touch, with the winding number at each base square's center.
/// An insideness test then only needs the few edges near its point, rather than the whole boundary.
struct ShapeGrid {
	edges: Vec<(DVec2, DVec2)>,
	min_x: i64,
	min_y: i64,
	columns: usize,
	rows: usize,
	/// Where each base square's run of indices into `edges` begins within `square_edges`, ending where the next square's run begins.
	square_edges_start: Vec<u32>,
	square_edges: Vec<u32>,
	center_winding: Vec<i32>,
}

impl ShapeGrid {
	/// Lists the `edges`, which lie within the bounds from `min` to `max`, by the base squares they touch.
	fn new(edges: Vec<(DVec2, DVec2)>, min: DVec2, max: DVec2, workload: &mut Workload) -> Result<Self, PoissonDiskError> {
		let (min_x, min_y) = (min.x.floor() as i64, min.y.floor() as i64);
		let columns = (max.x.floor() as i64 - min_x + 1) as usize;
		let rows = (max.y.floor() as i64 - min_y + 1) as usize;
		let square_count = columns * rows;
		let touched_squares = |edge| {
			touched_spans(edge, min_x, min_y, columns, rows)
				.flat_map(move |(row, first_column, last_column)| (first_column..=last_column).map(move |column| (row - min_y) as usize * columns + (column - min_x) as usize))
		};

		// Group the edges by base square with a counting sort, refusing too many listings before storing any
		let mut square_edges_start = vec![0_u32; square_count + 1];
		let mut listings = 0;
		for edge in &edges {
			for square in touched_squares(edge) {
				square_edges_start[square] += 1;
				listings += 1;
			}
			if workload.edge_listings + listings > MAX_EDGE_LISTINGS {
				return Err(PoissonDiskError::TooMuchDetail);
			}
		}
		workload.edge_listings += listings;

		// Running totals put each square's entry at the end of its run, which placing its edges then counts back down to the start
		let mut total = 0;
		for entry in &mut square_edges_start {
			total += *entry;
			*entry = total;
		}
		let mut square_edges = vec![0; listings];
		for (edge_index, edge) in edges.iter().enumerate() {
			for square in touched_squares(edge) {
				square_edges_start[square] -= 1;
				square_edges[square_edges_start[square] as usize] = edge_index as u32;
			}
		}

		let mut shape = Self {
			edges,
			min_x,
			min_y,
			columns,
			rows,
			square_edges_start,
			square_edges,
			center_winding: vec![0; square_count],
		};
		shape.compute_center_winding();
		Ok(shape)
	}

	/// Sweeps along each row's center line from the left, where the winding number is 0, adding the signed crossings of the edges.
	fn compute_center_winding(&mut self) {
		for row in 0..self.rows {
			let center_y = (self.min_y + row as i64) as f64 + 0.5;
			let mut winding_entering_square = 0;

			for column in 0..self.columns {
				let square = row * self.columns + column;
				let center = coordinate(DVec2::new((self.min_x + column as i64) as f64 + 0.5, center_y));
				let mut winding_at_center = winding_entering_square;

				for &edge in self.edges_in(square) {
					let (start, end) = self.edges[edge as usize];
					if (start.y <= center_y) == (end.y <= center_y) {
						continue;
					}

					// Count each crossing only in the square it falls in, since the edge may also be listed in its neighbors
					let crossing_x = start.x + (center_y - start.y) * (end.x - start.x) / (end.y - start.y);
					let crossing_column = (crossing_x.floor() as i64 - self.min_x).clamp(0, self.columns as i64 - 1) as usize;
					if crossing_column != column {
						continue;
					}

					// Moving toward +X across an edge heading toward +Y lowers the winding number
					let heading_toward_positive_y = end.y > start.y;
					let change = if heading_toward_positive_y { -1 } else { 1 };
					winding_entering_square += change;

					let center_side = orient(coordinate(start), coordinate(end), center);
					let crossing_before_center = if heading_toward_positive_y { center_side < 0. } else { center_side > 0. };
					if crossing_before_center {
						winding_at_center += change;
					}
				}

				self.center_winding[square] = winding_at_center;
			}
		}
	}

	/// The top-left corner of the base square at this column and row of the grid.
	#[inline]
	fn corner(&self, column: usize, row: usize) -> DVec2 {
		DVec2::new((self.min_x + column as i64) as f64, (self.min_y + row as i64) as f64)
	}

	#[inline]
	fn edges_in(&self, square: usize) -> &[u32] {
		&self.square_edges[self.square_edges_start[square] as usize..self.square_edges_start[square + 1] as usize]
	}

	/// The column and row of each base square not outside the shape, and whether it's entirely inside, in rows from the top-left.
	fn active_base_squares(&self) -> impl Iterator<Item = (usize, usize, bool)> {
		(0..self.rows).flat_map(move |row| (0..self.columns).map(move |column| (column, row))).filter_map(|(column, row)| {
			let square = row * self.columns + column;
			let inside = if !self.edges_in(square).is_empty() {
				false
			} else if self.center_winding[square] != 0 {
				true
			} else {
				return None;
			};
			Some((column, row, inside))
		})
	}

	/// The winding number at an offset from the top-left corner of the base square at this column and row.
	/// Adds the number of edges it examines to `edge_tests`.
	#[inline]
	fn winding(&self, column: usize, row: usize, offset: DVec2, edge_tests: &mut usize) -> i32 {
		self.winding_at(column, row, self.corner(column, row) + offset, edge_tests)
	}

	/// The winding number at a point in the base square at this column and row, from the square center's winding number and the edges crossed on the way to the point.
	/// Adds the number of edges it examines to `edge_tests`.
	fn winding_at(&self, column: usize, row: usize, point: DVec2, edge_tests: &mut usize) -> i32 {
		let base_square = row * self.columns + column;
		*edge_tests += self.edges_in(base_square).len();

		let center_position = self.corner(column, row) + 0.5;
		let (low, high) = (center_position.min(point), center_position.max(point));
		let center = coordinate(center_position);
		let point = coordinate(point);

		// Ties are settled with the center nudged a hair toward -X (and a far smaller hair toward +Y), which is where the row sweep counted its winding number
		let on_positive_side_of_line_to_point = |vertex: Coord<f64>| {
			let side = orient(center, point, vertex);
			side > 0. || (side == 0. && (point.y < vertex.y || (point.y == vertex.y && point.x < vertex.x)))
		};
		let center_on_positive_side_of_edge = |start: Coord<f64>, end: Coord<f64>| {
			let side = orient(start, end, center);
			side > 0. || (side == 0. && (end.y > start.y || (end.y == start.y && end.x > start.x)))
		};

		let mut winding = self.center_winding[base_square];
		for &edge in self.edges_in(base_square) {
			let (start, end) = self.edges[edge as usize];

			// An edge whose bounds miss those of the line from the center to the point can't cross it
			if start.x.max(end.x) < low.x || start.x.min(end.x) > high.x || start.y.max(end.y) < low.y || start.y.min(end.y) > high.y {
				continue;
			}
			let (start, end) = (coordinate(start), coordinate(end));

			// They cross if the edge's ends lie on opposite sides of the line from the center to the point, and that line's ends on opposite sides of the edge
			if on_positive_side_of_line_to_point(start) == on_positive_side_of_line_to_point(end) {
				continue;
			}
			let point_side = orient(start, end, point);
			if center_on_positive_side_of_edge(start, end) == (point_side > 0.) {
				continue;
			}

			winding += if point_side > 0. { 1 } else { -1 };
		}

		winding
	}

	/// Whether a square within the base square at this column and row, given by its corner's offset from the base square's corner and its size, is inside, outside, or straddling the shape's boundary (meaning any edge touches it).
	/// Adds the number of edges it examines to `edge_tests`.
	fn coverage(&self, column: usize, row: usize, offset: DVec2, size: f64, edge_tests: &mut usize) -> Coverage {
		let base_square = row * self.columns + column;
		*edge_tests += self.edges_in(base_square).len();
		let min = self.corner(column, row) + offset;
		let max = min + size;
		let corners = [min, DVec2::new(max.x, min.y), max, DVec2::new(min.x, max.y)].map(coordinate);

		let touched = self.edges_in(base_square).iter().any(|&edge| {
			let (start, end) = self.edges[edge as usize];
			if start.x.max(end.x) < min.x || start.x.min(end.x) > max.x || start.y.max(end.y) < min.y || start.y.min(end.y) > max.y {
				return false;
			}
			let (start, end) = (coordinate(start), coordinate(end));

			// Touching unless all four corners lie strictly to one side of the edge's line, so the first corner on that line or across it settles it
			let first_side = orient(start, end, corners[0]);
			first_side == 0.
				|| corners[1..].iter().any(|&corner| {
					let side = orient(start, end, corner);
					side == 0. || (side > 0.) != (first_side > 0.)
				})
		});

		if touched {
			Coverage::Straddling
		} else if self.winding_at(column, row, min + size / 2., edge_tests) != 0 {
			Coverage::Inside
		} else {
			Coverage::Outside
		}
	}
}

/// The points accepted so far, each stored in the slot of its base square (since none can hold two) as its offset from the base square's top-left corner.
struct AcceptedPoints {
	/// The offsets, which are exact as `f32`s, or NaN where a slot holds no point.
	slots: Vec<[f32; 2]>,
	/// One bit per slot that holds a point, in the slots' order, with a spare word so two neighboring words can always be read.
	occupied: Vec<u64>,
	min_x: i64,
	min_y: i64,
	columns: usize,
}

impl AcceptedPoints {
	/// Room for points in the shape's base squares, plus the two-square margin that neighborhood lookups reach into.
	fn new(shape: &ShapeGrid) -> Self {
		let columns = shape.columns + 4;
		let rows = shape.rows + 4;

		Self {
			slots: vec![[f32::NAN; 2]; columns * rows],
			occupied: vec![0; columns * rows / 64 + 2],
			min_x: shape.min_x - 2,
			min_y: shape.min_y - 2,
			columns,
		}
	}

	/// The slot of the base square at this column and row of the shape's grid.
	#[inline]
	fn slot(&self, column: usize, row: usize) -> usize {
		(row + 2) * self.columns + column + 2
	}

	/// The accepted points that could be within the separation of anything in a square of the base square at this column and row, given by its corner and size within that base square.
	/// They're placed relative to the base square's top-left corner, where the arithmetic on them is exact.
	#[inline]
	fn nearby(&self, column: usize, row: usize, square_min: DVec2, square_size: f64) -> NearbyPoints {
		let mut nearby = NearbyPoints { points: [DVec2::ZERO; 25], count: 0 };

		// Base squares two away only come within the separation of a square near their side of its base square
		let reach = |center: usize, min: f64| {
			let gap_to_first = min + 1.;
			let gap_to_last = 2. - (min + square_size);
			let first = if gap_to_first * gap_to_first <= SEPARATION_SQUARED { center - 2 } else { center - 1 };
			let last = if gap_to_last * gap_to_last <= SEPARATION_SQUARED { center + 2 } else { center + 1 };
			first..=last
		};
		let (x, y) = (column + 2, row + 2);
		let (columns, rows) = (reach(x, square_min.x), reach(y, square_min.y));

		// Each row's occupancy bits across the window go into a byte of one word, so a single loop visits just the slots holding points
		let first_column = *columns.start();
		let first_row = *rows.start();
		let window = (1_u64 << (columns.end() - first_column + 1)) - 1;
		let mut holding = 0_u64;
		for row in rows {
			let first = row * self.columns + first_column;
			let (word, shift) = (first / 64, first % 64);
			let row_bits = (self.occupied[word] >> shift) | ((self.occupied[word + 1] << 1) << (63 - shift));
			holding |= (row_bits & window) << (8 * (row - first_row));
		}

		while holding != 0 {
			let bit = holding.trailing_zeros() as usize;
			holding &= holding - 1;

			let (row, column) = (first_row + bit / 8, first_column + bit % 8);
			let [offset_x, offset_y] = self.slots[row * self.columns + column];
			nearby.points[nearby.count] = DVec2::new(column as f64 - x as f64 + offset_x as f64, row as f64 - y as f64 + offset_y as f64);
			nearby.count += 1;
		}

		nearby
	}

	#[inline]
	fn holds_point(&self, column: usize, row: usize) -> bool {
		let slot = self.slot(column, row);
		(self.occupied[slot / 64] >> (slot % 64)) & 1 != 0
	}

	/// Adds a point at the given offset from its base square's top-left corner, which must be a multiple of the dart grid's step.
	#[inline]
	fn insert(&mut self, offset: DVec2, column: usize, row: usize) {
		let slot = self.slot(column, row);
		self.slots[slot] = [offset.x as f32, offset.y as f32];
		self.occupied[slot / 64] |= 1 << (slot % 64);
	}

	/// The accepted points in lattice units, each with the address of its base square and its hash, in rows of base squares from the top-left.
	fn into_points(self, lattice: &DartLattice) -> impl Iterator<Item = ((i64, i64), DVec2, u64)> {
		let Self { slots, min_x, min_y, columns, .. } = self;
		slots.into_iter().enumerate().filter(|(_, [offset_x, _])| !offset_x.is_nan()).map(move |(index, offset)| {
			let (base_x, base_y) = (min_x + (index % columns) as i64, min_y + (index / columns) as i64);
			let point = DVec2::new(base_x as f64 + offset[0] as f64, base_y as f64 + offset[1] as f64);
			((base_x, base_y), point, lattice.point_hash(base_x, base_y, offset))
		})
	}
}

/// The accepted points near one base square, gathered once for testing a dart and the sub-squares it may leave.
struct NearbyPoints {
	points: [DVec2; 25],
	count: usize,
}

impl NearbyPoints {
	#[inline]
	fn covers_point(&self, point: DVec2) -> bool {
		self.points[..self.count].iter().any(|nearby| nearby.distance_squared(point) <= SEPARATION_SQUARED)
	}

	/// Whether one accepted point covers the whole square, by reaching its farthest corner.
	#[inline]
	fn covers_square(&self, min: DVec2, size: f64) -> bool {
		let half_size = size / 2.;
		let center = min + half_size;
		self.points[..self.count]
			.iter()
			.any(|nearby| ((center - *nearby).abs() + half_size).length_squared() <= SEPARATION_SQUARED)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use kurbo::{Circle, Rect, Shape};

	fn polygon_edges(vertices: &[DVec2]) -> Vec<(DVec2, DVec2)> {
		(0..vertices.len()).map(|i| (vertices[i], vertices[(i + 1) % vertices.len()])).collect()
	}

	fn star(center: DVec2, radius: f64, points: usize) -> Vec<DVec2> {
		(0..points)
			.map(|i| center + DVec2::from_angle(i as f64 * std::f64::consts::TAU * 2. / points as f64) * radius)
			.collect()
	}

	// Self-overlapping stars, one reversed, and a triangle overlapping both, away from the origin
	#[test]
	fn shape_grid_winding_matches_kurbo() {
		let mut hole = star(DVec2::new(1000.3, -500.2), 12., 5);
		hole.reverse();
		let loops = [
			star(DVec2::new(1000.3, -500.2), 40., 7),
			hole,
			vec![DVec2::new(990., -520.), DVec2::new(1010., -520.), DVec2::new(1010., -480.)],
		];

		let mut bezpath = BezPath::new();
		let mut edges = Vec::new();
		for vertices in &loops {
			bezpath.move_to((vertices[0].x, vertices[0].y));
			for vertex in &vertices[1..] {
				bezpath.line_to((vertex.x, vertex.y));
			}
			bezpath.close_path();
			edges.extend(polygon_edges(vertices));
		}

		let lattice = DartLattice::new(3., 7);
		let lattice_edges: Vec<_> = edges.iter().map(|&(start, end)| (lattice.to_lattice(start), lattice.to_lattice(end))).collect();
		let min = lattice_edges.iter().fold(DVec2::INFINITY, |min, &(start, end)| min.min(start).min(end));
		let max = lattice_edges.iter().fold(DVec2::NEG_INFINITY, |max, &(start, end)| max.max(start).max(end));
		let shape = ShapeGrid::new(lattice_edges, min, max, &mut Workload::default()).unwrap();

		let mut nonzero_count = 0;
		for i in 0..20_000 {
			let hash = mix(i);
			let probe = DVec2::new(950. + unit_interval(mix(hash ^ 1)) * 100., -550. + unit_interval(mix(hash ^ 2)) * 100.);

			// Points within rounding error of an edge have no reliable answer
			let near_edge = edges.iter().any(|&(start, end)| {
				let t = ((probe - start).dot(end - start) / (end - start).length_squared()).clamp(0., 1.);
				probe.distance(start + (end - start) * t) < 1e-9
			});
			if near_edge {
				continue;
			}

			let expected = bezpath.winding(kurbo::Point::new(probe.x, probe.y));
			let lattice_probe = lattice.to_lattice(probe);
			let (x, y) = (lattice_probe.x.floor() as i64, lattice_probe.y.floor() as i64);
			let in_grid = x >= shape.min_x && y >= shape.min_y && x < shape.min_x + shape.columns as i64 && y < shape.min_y + shape.rows as i64;
			let winding = if in_grid {
				shape.winding_at((x - shape.min_x) as usize, (y - shape.min_y) as usize, lattice_probe, &mut 0)
			} else {
				0
			};

			assert_eq!(winding, expected, "winding at {probe}");
			nonzero_count += (winding != 0) as usize;
		}
		assert!(nonzero_count > 1000, "too few probes landed inside the shape: {nonzero_count}");
	}

	// A rectangle in lattice units with its top-right corner, and the sides meeting there, through base square centers
	#[test]
	fn shape_grid_winding_holds_with_edges_through_square_centers() {
		let edges = polygon_edges(&[DVec2::new(0.2, 0.5), DVec2::new(2.5, 0.5), DVec2::new(2.5, 3.8), DVec2::new(0.2, 3.8)]);
		let shape = ShapeGrid::new(edges, DVec2::new(0.2, 0.5), DVec2::new(2.5, 3.8), &mut Workload::default()).unwrap();
		let winding = |point: DVec2| shape.winding_at((point.x.floor() as i64 - shape.min_x) as usize, (point.y.floor() as i64 - shape.min_y) as usize, point, &mut 0);

		for inside in [(1.6, 0.6), (2.4, 1.6), (2.4, 0.6), (2.45, 0.55), (2.1, 0.9), (2.3, 2.6)] {
			assert_ne!(winding(inside.into()), 0, "{inside:?} should be inside");
		}
		for outside in [(1.6, 0.4), (2.6, 1.6), (2.6, 0.4), (2.4, 0.4), (2.45, 0.3), (2.1, 0.45), (2.3, 0.1), (2.6, 0.6), (2.9, 0.1)] {
			assert_eq!(winding(outside.into()), 0, "{outside:?} should be outside");
		}
	}

	#[test]
	fn points_keep_their_separation_and_fill_the_shape() {
		// A rectangle with a hole, its winding reversed to cancel the rectangle's
		let separation = 5.;
		let (hole_center, hole_radius) = (DVec2::new(100., 60.), 30.);
		let mut shape = Rect::new(0., 0., 200., 120.).to_path(0.1);
		shape.extend(Circle::new((hole_center.x, hole_center.y), hole_radius).to_path(1e-3).reverse_subpaths());
		let sampled = poisson_disk_points(std::slice::from_ref(&shape), separation, 3).unwrap();
		let points: Vec<_> = sampled.iter().map(|&(point, _)| point).collect();

		// The same input gives the same points, each with its own hash
		assert_eq!(sampled, poisson_disk_points(&[shape], separation, 3).unwrap());
		let hashes: std::collections::HashSet<_> = sampled.iter().map(|&(_, hash)| hash).collect();
		assert_eq!(hashes.len(), sampled.len());

		// Points this close to the hole's boundary may fall to either side of its straight-line approximation
		let near_hole_boundary = |point: DVec2| (point.distance(hole_center) - hole_radius).abs() < 0.05;
		let in_shape = |point: DVec2| point.x >= 0. && point.x <= 200. && point.y >= 0. && point.y <= 120. && point.distance(hole_center) >= hole_radius;

		for (i, a) in points.iter().enumerate() {
			assert!(in_shape(*a) || near_hole_boundary(*a), "{a} lies outside the shape");
			for b in &points[i + 1..] {
				assert!(a.distance(*b) > separation * (1. - 1e-9), "{a} and {b} are closer than the separation");
			}
		}

		// Every probe inside the shape is within the separation of some point, so no more points could fit
		for i in 0..10_000 {
			let hash = mix(i);
			let probe = DVec2::new(unit_interval(mix(hash ^ 1)) * 200., unit_interval(mix(hash ^ 2)) * 120.);
			if in_shape(probe) && !near_hole_boundary(probe) {
				assert!(points.iter().any(|point| point.distance(probe) <= separation), "{probe} is uncovered");
			}
		}
	}

	fn path(vertices: &[(f64, f64)]) -> BezPath {
		let mut bezpath = BezPath::new();
		bezpath.move_to(vertices[0]);
		for &vertex in &vertices[1..] {
			bezpath.line_to(vertex);
		}
		bezpath
	}

	#[test]
	fn boundaries_enclosing_no_area_add_no_points() {
		assert!(poisson_disk_points(&[], 5., 0).unwrap().is_empty());
		assert!(poisson_disk_points(&[BezPath::new()], 5., 0).unwrap().is_empty());

		let open_line = path(&[(0., 0.), (800., 600.)]);
		assert!(poisson_disk_points(&[open_line], 5., 0).unwrap().is_empty());

		// A line far too long to lay a grid over at this separation doesn't stop the square beside it from being sampled
		let square = path(&[(0., 0.), (100., 0.), (100., 100.), (0., 100.)]);
		let long_line = path(&[(-1e5, -1e5), (1e5, 1e5)]);
		assert_eq!(poisson_disk_points(&[square.clone(), long_line], 5., 0).unwrap(), poisson_disk_points(&[square], 5., 0).unwrap());

		// Rounding leaves the middle point a hair off the line
		let middle = DVec2::ZERO.lerp(DVec2::new(800., 600.), 0.37);
		let subdivided_open_line = path(&[(0., 0.), (middle.x, middle.y), (800., 600.)]);
		assert!(poisson_disk_points(&[subdivided_open_line], 5., 0).unwrap().is_empty());

		// A zigzag traced out and then back along itself
		let mut zigzag: Vec<_> = (0..=100).map(|i| (i as f64, if i % 2 == 0 { 0. } else { 30. })).collect();
		zigzag.extend(zigzag.clone().into_iter().rev());
		assert!(poisson_disk_points(&[path(&zigzag)], 10., 0).unwrap().is_empty());

		// The spike runs out from the square and back along the same line
		let square = path(&[(0., 0.), (100., 0.), (100., 100.), (0., 100.)]);
		let spiked_square = path(&[(0., 0.), (100., 0.), (100., 50.), (1000., 50.), (100., 50.), (100., 100.), (0., 100.)]);
		assert_eq!(poisson_disk_points(&[spiked_square], 5., 0).unwrap(), poisson_disk_points(&[square], 5., 0).unwrap());
	}

	// Two squares overlapping in the same direction, giving a winding number of 2 where they overlap
	#[test]
	fn overlapping_subpaths_fill_their_union() {
		let separation = 5.;
		let shape = [path(&[(0., 0.), (100., 0.), (100., 100.), (0., 100.)]), path(&[(50., 50.), (150., 50.), (150., 150.), (50., 150.)])];
		let points: Vec<_> = poisson_disk_points(&shape, separation, 1).unwrap().into_iter().map(|(point, _)| point).collect();
		let in_union = |point: DVec2| (point.cmpge(DVec2::ZERO).all() && point.cmple(DVec2::splat(100.)).all()) || (point.cmpge(DVec2::splat(50.)).all() && point.cmple(DVec2::splat(150.)).all());

		assert!(points.iter().all(|&point| in_union(point)));
		for i in 0..5000 {
			let hash = mix(i);
			let probe = DVec2::new(unit_interval(mix(hash ^ 1)), unit_interval(mix(hash ^ 2))) * 150.;
			if in_union(probe) {
				assert!(points.iter().any(|point| point.distance(probe) <= separation), "{probe} is uncovered");
			}
		}
	}

	// Squares far enough apart to be sampled on separate grids, side by side so their rows interleave
	#[test]
	fn points_come_in_rows_across_separate_parts() {
		let (separation, seed) = (5., 2);
		let shape = [path(&[(0., 0.), (50., 0.), (50., 50.), (0., 50.)]), path(&[(100., 0.), (150., 0.), (150., 50.), (100., 50.)])];
		let points = poisson_disk_points(&shape, separation, seed).unwrap();

		let lattice = DartLattice::new(separation, seed);
		let addresses: Vec<_> = points.iter().map(|&(point, _)| lattice.to_lattice(point).floor()).map(|address| (address.y, address.x)).collect();
		assert!(points.iter().any(|(point, _)| point.x < 50.) && points.iter().any(|(point, _)| point.x > 100.));
		assert!(addresses.windows(2).all(|pair| pair[0] < pair[1]), "points aren't in rows of base squares");
	}

	#[test]
	fn separations_without_a_meaningful_placement_give_no_points() {
		let circle = Circle::new((0., 0.), 50.).to_path(0.1);
		for separation in [f64::INFINITY, f64::NAN, 0., -1.] {
			assert!(
				poisson_disk_points(std::slice::from_ref(&circle), separation, 0).unwrap().is_empty(),
				"separation {separation} placed points"
			);
		}
	}

	// The ring's grid fits within the limit once, but not twice over
	#[test]
	fn limits_hold_across_shapes_sharing_a_sampler() {
		let mut ring = Circle::new((0., 0.), 1500.).to_path(1e-3);
		ring.extend(Circle::new((0., 0.), 1495.).to_path(1e-3).reverse_subpaths());
		let ring = [ring];

		let mut sampler = PoissonDiskSampler::default();
		assert!(sampler.points(&ring, 1.5, 0).is_ok());
		assert!(matches!(sampler.points(&ring, 1.5, 0), Err(PoissonDiskError::ExtentTooLarge)));
		assert!(poisson_disk_points(&ring, 1.5, 0).is_ok());
	}

	// Together the squares need too many points, which is found before any darts are thrown at either
	#[test]
	fn shapes_needing_too_many_points_are_refused_before_any_darts() {
		let squares = [path(&[(0., 0.), (990., 0.), (990., 990.), (0., 990.)]), path(&[(2000., 0.), (2200., 0.), (2200., 200.), (2000., 200.)])];
		let mut sampler = PoissonDiskSampler::default();
		assert!(matches!(sampler.points(&squares, 1., 0), Err(PoissonDiskError::SeparationTooSmall { .. })));
		assert_eq!(sampler.workload.darts, 0);
	}

	// Each stroke of the zigzag runs millions of base squares long, within a sliver of a single row
	#[test]
	fn detail_far_finer_than_the_separation_is_refused() {
		let zigzag: Vec<_> = (0..=8).map(|i| (if i % 2 == 0 { 0. } else { 2_900_000. }, 0.5 * i as f64 / 8.)).collect();
		assert!(matches!(poisson_disk_points(&[path(&zigzag)], 1., 0), Err(PoissonDiskError::TooMuchDetail)));
	}

	#[test]
	fn different_seeds_give_different_points() {
		let square = [path(&[(0., 0.), (100., 0.), (100., 100.), (0., 100.)])];
		assert_ne!(poisson_disk_points(&square, 5., 0).unwrap(), poisson_disk_points(&square, 5., 1).unwrap());
	}

	// Saved documents depend on their seeds placing the same points, so any change to them must be deliberate (and the demo artwork's seeds searched for again)
	// The smaller separation gives the shape enough base squares for the queue to hold them in its radix heap rather than its binary heap
	#[test]
	fn points_are_unchanged() {
		let mut blob = BezPath::new();
		blob.move_to((0., 0.));
		blob.curve_to((50., -80.), (150., 80.), (200., 0.));
		blob.quad_to((260., 100.), (200., 200.));
		blob.curve_to((150., 120.), (50., 280.), (0., 200.));
		blob.close_path();
		blob.extend(path(&[(60., 60.), (60., 120.), (120., 90.)]));

		for (separation, expected) in [(7., (637, 0x29FD_41EF_9919_D1F2)), (1., (29_672, 0x346F_9F84_1ACA_8D29))] {
			let points = poisson_disk_points(std::slice::from_ref(&blob), separation, 42).unwrap();
			let fingerprint = points
				.iter()
				.fold(0, |fingerprint, &(point, hash)| mix(fingerprint ^ point.x.to_bits()) ^ mix(point.y.to_bits() ^ hash));
			assert_eq!(
				(points.len(), fingerprint),
				expected,
				"the points at separation {separation} changed, giving a fingerprint of {fingerprint:#018X}"
			);
		}
	}

	// Many squares arrive at equal times, and each one given out may add a later one, as a dart's sub-squares do
	#[test]
	fn arrival_queue_orders_squares_alike_for_few_and_many_base_squares() {
		let orders = [FEW_BASE_SQUARES, FEW_BASE_SQUARES + 1].map(|base_squares| {
			let mut queue = ArrivalQueue::default();
			queue.clear(base_squares);
			for i in 0..2000 {
				queue.push(ActiveSquare {
					time: (mix(i) % 50) as f64,
					packed: mix(i ^ 0xFF),
				});
			}

			let mut order = Vec::new();
			while let Some(square) = queue.pop() {
				order.push((square.time.to_bits(), square.packed));
				let step = order.len() as u64;
				if step <= 3000 {
					queue.push(ActiveSquare {
						time: square.time + (1 + mix(step) % 8) as f64,
						packed: mix(step ^ 0xFFFF),
					});
				}
			}
			order
		});

		assert_eq!(orders[0].len(), 5000);
		assert!(orders[0].windows(2).all(|pair| pair[0] < pair[1]), "squares came out of order");
		assert_eq!(orders[0], orders[1]);
	}
}
