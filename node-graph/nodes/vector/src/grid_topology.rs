use core_types::list::{ATTR_ID, Item, List};
use glam::DVec2;
use graphic_types::Vector;
use rustc_hash::FxHashMap;
use std::collections::VecDeque;
use vector_types::vector::misc::{GridTopology, GridType};
use vector_types::vector::{CurveType, Path, PointId, SegmentId, SetVectorPaths, push_paths};

/// A point's column and its height in half rows. Even columns sit at even heights, and isometric odd columns at the odd heights between them.
type Coordinate = (i64, i64);

/// Which edges Staggered keeps, leaving each point with three of them.
#[derive(Clone, Copy)]
enum Stagger {
	/// A brick wall, where every row stays whole and the vertical edges alternate from one row to the next.
	Bricks,
	/// A honeycomb around the points whose [`third`] is `center`, each of which loses its edges to become a hexagon's center.
	Honeycomb { center: i64 },
}

/// One path through a layout's points.
pub(crate) struct GridPath {
	points: Vec<usize>,
	closed: bool,
}

impl GridPath {
	fn open(points: Vec<usize>) -> Self {
		Self { points, closed: false }
	}

	fn closed(points: Vec<usize>) -> Self {
		Self { points, closed: true }
	}
}

/// A grid's points and the paths joining them in one topology.
pub(crate) struct GridLayout {
	positions: Vec<DVec2>,
	paths: Vec<GridPath>,
	/// Whether each point stays in the vector even with no path running through it, which only Points Only wants. Missing entries count as false.
	kept_free_points: Vec<bool>,
}

impl GridLayout {
	/// Builds the vector, numbering the points and then the segments in the order they are laid out.
	pub fn into_item(self) -> Item<Vector> {
		let grid_paths: Vec<GridPath> = self.paths.into_iter().filter(|path| path.points.len() > 1).collect();

		let mut kept = self.kept_free_points;
		kept.resize(self.positions.len(), false);
		for &point in grid_paths.iter().flat_map(|path| &path.points) {
			if let Some(kept) = kept.get_mut(point) {
				*kept = true;
			}
		}

		// Points take their IDs from their place in the layout, so leaving out free points renumbers none of the rest
		let mut point_id = PointId::ZERO;
		let (mut positions, mut point_ids) = (Vec::new(), Vec::new());
		let mut kept_index = vec![None; kept.len()];
		for (index, position) in self.positions.into_iter().enumerate() {
			let id = point_id.next_id();
			if kept[index] {
				kept_index[index] = Some(positions.len() as u32);
				positions.push(position);
				point_ids.push(id);
			}
		}

		let mut segment_id = SegmentId::ZERO;
		let runs: Vec<(CurveType, Vec<u32>, Vec<SegmentId>)> = grid_paths
			.into_iter()
			.map(|path| {
				let mut references: Vec<u32> = path.points.iter().filter_map(|&point| kept_index.get(point).copied().flatten()).collect();
				if path.closed && references.len() > 2 {
					references.push(references[0]);
				}
				let ids = (1..references.len()).map(|_| segment_id.next_id()).collect();

				(CurveType::Linear, references, ids)
			})
			.collect();

		let mut vector = Vector(List::new_from_elements(positions));
		if !point_ids.is_empty() {
			vector.0.set_attribute_values(ATTR_ID, point_ids);
		}

		let mut paths = List::<Path>::new();
		push_paths(&mut paths, runs);

		Item::new_from_element(vector).with_paths(paths)
	}
}

/// The grid's points and the edges between neighboring points.
pub(crate) struct Lattice {
	grid_type: GridType,
	columns: i64,
	rows: i64,
	/// Each point's position, stored row by row from the top and left to right along each row.
	positions: Vec<DVec2>,
	coordinates: Vec<Coordinate>,
	/// Each column's points from top to bottom.
	column_points: Vec<Vec<usize>>,
	/// The height of each column's top point.
	column_tops: Vec<i64>,
	stagger: Option<Stagger>,
}

impl Lattice {
	/// Lays out the grid's points, placing each with `position` from its column and row.
	/// A row of an isometric odd column sits half a row above the same row of an even column.
	pub fn new(grid_type: GridType, columns: u32, rows: u32, staggered: bool, position: impl Fn(u32, i64) -> DVec2) -> Self {
		let isometric = grid_type.is_isometric();
		let (columns, rows) = (columns as i64, rows as i64);

		// Odd isometric columns start half a row up with the same count (Tiling) or one more (Symmetric +1), or half a row down with one fewer (Symmetric −1)
		let (column_tops, column_counts): (Vec<i64>, Vec<i64>) = (0..columns)
			.map(|x| {
				if x % 2 == 0 {
					return (0, rows);
				}
				match grid_type {
					GridType::Rectangular => (0, rows),
					GridType::IsometricTiling => (-1, rows),
					GridType::IsometricSymmetricPlusOne => (-1, rows + 1),
					GridType::IsometricSymmetricMinusOne => (1, (rows - 1).max(0)),
				}
			})
			.unzip();

		let mut lattice = Self {
			grid_type,
			columns,
			rows,
			positions: Vec::new(),
			coordinates: Vec::new(),
			column_points: column_counts.iter().map(|&count| vec![usize::MAX; count as usize]).collect(),
			column_tops,
			stagger: None,
		};

		// Symmetric +1 stores the extra bottom points of its odd columns in a row of their own
		let last_row = if grid_type == GridType::IsometricSymmetricPlusOne { rows } else { rows - 1 };
		for row in 0..=last_row {
			for x in 0..columns {
				let height = if isometric && x % 2 == 1 { 2 * row - 1 } else { 2 * row };
				let offset = height - lattice.column_tops[x as usize];
				if offset < 0 || offset / 2 >= column_counts[x as usize] {
					continue;
				}

				lattice.column_points[x as usize][(offset / 2) as usize] = lattice.positions.len();
				lattice.coordinates.push((x, height));
				lattice.positions.push(position(x as u32, row));
			}
		}

		if staggered {
			lattice.stagger = Some(if isometric { Stagger::Honeycomb { center: lattice.honeycomb_center() } } else { Stagger::Bricks });
		}

		lattice
	}

	/// Joins the points into paths by the topology.
	pub fn layout(&self, topology: GridTopology) -> GridLayout {
		let open = |trails: Vec<Vec<usize>>| -> Vec<GridPath> { trails.into_iter().map(GridPath::open).collect() };

		let paths = match topology {
			GridTopology::Lines => open(self.line_trails()),
			GridTopology::Edges => self.edges().into_iter().map(|(start, end)| GridPath::open(vec![start, end])).collect(),
			GridTopology::FewestPaths => fewest_trails(&self.positions, &self.edges()),
			GridTopology::SinglePath => self.single_path(),
			GridTopology::RowsOnly => open(self.row_trails()),
			GridTopology::ColumnsOnly => open(self.walk(|(x, height)| (x, height + 2), |_| true)),
			GridTopology::PointsOnly => return self.points_only(),
			GridTopology::Serpentine => self.serpentine(),
			GridTopology::HilbertCurve => self.hilbert_curve(),
			GridTopology::Spiral => self.spiral(),
			GridTopology::Loop => match self.loop_route() {
				Some(route) => vec![GridPath::closed(route)],
				None => self.serpentine(),
			},
			GridTopology::Rings => self.rings().0,
			GridTopology::Cells => return self.separate_polygons(self.cells()),
			GridTopology::Triangles => return self.separate_polygons(self.triangles()),
		};

		GridLayout {
			positions: self.positions.clone(),
			paths,
			kept_free_points: Vec::new(),
		}
	}

	fn isometric(&self) -> bool {
		self.grid_type.is_isometric()
	}

	/// The steps from a point to its neighbors on the right and below.
	fn steps(&self) -> &'static [Coordinate] {
		if self.isometric() { &[(0, 2), (1, -1), (1, 1)] } else { &[(1, 0), (0, 2)] }
	}

	/// The point at a coordinate, if the grid has one there.
	fn at(&self, (x, height): Coordinate) -> Option<usize> {
		if !(0..self.columns).contains(&x) {
			return None;
		}

		let offset = height - self.column_tops[x as usize];
		if offset < 0 || offset % 2 != 0 {
			return None;
		}

		self.column_points[x as usize].get((offset / 2) as usize).copied()
	}

	/// Whether Staggered keeps the edge between two neighboring points.
	fn keeps_edge(&self, a: usize, b: usize) -> bool {
		let ((a_x, a_height), (b_x, b_height)) = (self.coordinates[a], self.coordinates[b]);

		match self.stagger {
			None => true,
			Some(Stagger::Bricks) => a_x != b_x || self.keeps_rung((a_x, a_height.min(b_height))),
			Some(Stagger::Honeycomb { center }) => third((a_x, a_height)) != center && third((b_x, b_height)) != center,
		}
	}

	/// Whether the brick wall keeps the vertical edge below a point, alternating along each row and between rows, and always at the sides.
	fn keeps_rung(&self, (x, height): Coordinate) -> bool {
		(x + height / 2) % 2 == 0 || x == 0 || x == self.columns - 1
	}

	/// The points sharing an edge with a point.
	fn neighbors(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
		let (x, height) = self.coordinates[index];

		self.steps()
			.iter()
			.flat_map(move |&(step_x, step_height)| [(x + step_x, height + step_height), (x - step_x, height - step_height)])
			.filter_map(move |coordinate| self.at(coordinate))
			.filter(move |&neighbor| self.keeps_edge(index, neighbor))
	}

	fn are_neighbors(&self, a: usize, b: usize) -> bool {
		let ((a_x, a_height), (b_x, b_height)) = (self.coordinates[a], self.coordinates[b]);
		let step = (b_x - a_x, b_height - a_height);

		self.steps().iter().any(|&(x, height)| step == (x, height) || step == (-x, -height)) && self.keeps_edge(a, b)
	}

	/// The set of honeycomb hexagon centers that leaves the fewest points with one edge or none.
	fn honeycomb_center(&self) -> i64 {
		let dangling = |center: i64| {
			let kept = |index: usize| third(self.coordinates[index]) != center;
			(0..self.positions.len())
				.filter(|&index| kept(index) && self.neighbors(index).filter(|&neighbor| kept(neighbor)).count() <= 1)
				.count()
		};

		(0..3).min_by_key(|&center| dangling(center)).unwrap_or(0)
	}

	/// Every edge Staggered keeps, in the order the grid has always stored them: each point's edges back to the neighbors stored before it.
	fn edges(&self) -> Vec<(usize, usize)> {
		let mut edges = Vec::new();

		for (index, &(x, height)) in self.coordinates.iter().enumerate() {
			// To the left, above, and for isometric odd columns, diagonally above on the right and then the left
			let backward_steps: &[Coordinate] = match (self.isometric(), x % 2 == 1) {
				(false, _) => &[(-1, 0), (0, -2)],
				(true, false) => &[(-1, -1), (0, -2)],
				(true, true) => &[(-1, 1), (0, -2), (1, -1), (-1, -1)],
			};

			for &(step_x, step_height) in backward_steps {
				if let Some(neighbor) = self.at((x + step_x, height + step_height))
					&& self.keeps_edge(neighbor, index)
				{
					edges.push((neighbor, index));
				}
			}
		}

		edges
	}

	/// Trails following `step` from each point that no other point steps to, for as long as it lands on points `included` accepts, keeping those spanning an edge.
	fn walk(&self, step: impl Fn(Coordinate) -> Coordinate, included: impl Fn(usize) -> bool) -> Vec<Vec<usize>> {
		let step_from = |index: usize| self.at(step(self.coordinates[index])).filter(|&next| included(next));

		let mut has_predecessor = vec![false; self.positions.len()];
		for index in (0..self.positions.len()).filter(|&index| included(index)) {
			if let Some(next) = step_from(index) {
				has_predecessor[next] = true;
			}
		}

		let mut trails = Vec::new();
		for start in (0..self.positions.len()).filter(|&index| included(index) && !has_predecessor[index]) {
			let mut trail = vec![start];
			let mut current = start;
			while let Some(next) = step_from(current) {
				trail.push(next);
				current = next;
			}

			if trail.len() > 1 {
				trails.push(trail);
			}
		}

		trails
	}

	/// The trails along each straight line through the grid, or along Staggered's rows and the rungs between them.
	fn line_trails(&self) -> Vec<Vec<usize>> {
		let Some(stagger) = self.stagger else {
			return self
				.steps()
				.iter()
				.flat_map(|&(step_x, step_height)| self.walk(|(x, height)| (x + step_x, height + step_height), |_| true))
				.collect();
		};

		let mut trails = self.staggered_rows(stagger);
		trails.extend((0..self.positions.len()).filter_map(|index| {
			let (x, height) = self.coordinates[index];
			let below = self.at((x, height + 2))?;
			self.keeps_edge(index, below).then(|| vec![index, below])
		}));

		trails
	}

	/// Staggered's rows: straight for bricks, and zigzagging around the hexagon centers for honeycombs.
	fn staggered_rows(&self, stagger: Stagger) -> Vec<Vec<usize>> {
		match stagger {
			Stagger::Bricks => self.walk(|(x, height)| (x + 1, height), |_| true),
			Stagger::Honeycomb { center } => {
				// A row descends from each point just past a center and climbs back from each point two past one
				let descends = move |coordinate: Coordinate| third(coordinate) == (center + 1) % 3;
				let step = |(x, height): Coordinate| (x + 1, if descends((x, height)) { height + 1 } else { height - 1 });

				self.walk(step, |index| third(self.coordinates[index]) != center)
			}
		}
	}

	/// One path along each row. Isometric rows zigzag up into odd columns on Tiling and down into them on the Symmetric shapes, so each row spans the grid.
	fn row_trails(&self) -> Vec<Vec<usize>> {
		match self.stagger {
			Some(stagger @ Stagger::Honeycomb { .. }) => self.staggered_rows(stagger),
			_ if !self.isometric() => self.walk(|(x, height)| (x + 1, height), |_| true),
			_ => {
				let phase = if self.grid_type == GridType::IsometricTiling { -1 } else { 1 };
				self.walk(|(x, height)| (x + 1, if x % 2 == 0 { height + phase } else { height - phase }), |_| true)
			}
		}
	}

	/// Just the points, except that Staggered isometric grids keep only the hexagon centers.
	fn points_only(&self) -> GridLayout {
		let kept_free_points = match self.stagger {
			Some(Stagger::Honeycomb { center }) => self.coordinates.iter().map(|&coordinate| third(coordinate) == center).collect(),
			_ => vec![true; self.positions.len()],
		};

		GridLayout {
			positions: self.positions.clone(),
			paths: Vec::new(),
			kept_free_points,
		}
	}

	/// One route back and forth through every point, along rectangular rows or isometric columns.
	fn serpentine(&self) -> Vec<GridPath> {
		let lines: Vec<Vec<usize>> = if self.isometric() {
			self.column_points.clone()
		} else {
			(0..self.rows as usize).map(|row| self.column_points.iter().map(|column| column[row]).collect()).collect()
		};

		let route = lines.into_iter().enumerate().flat_map(|(index, mut line)| {
			if index % 2 == 1 {
				line.reverse();
			}
			line
		});

		self.split_where_unconnected(route)
	}

	/// Splits a route into open paths wherever two consecutive points share no edge.
	fn split_where_unconnected(&self, route: impl IntoIterator<Item = usize>) -> Vec<GridPath> {
		let mut pieces = vec![Vec::new()];

		for point in route {
			if let Some(&previous) = pieces.last().and_then(|piece: &Vec<usize>| piece.last())
				&& !self.are_neighbors(previous, point)
			{
				pieces.push(Vec::new());
			}
			if let Some(piece) = pieces.last_mut() {
				piece.push(point);
			}
		}

		pieces.into_iter().filter(|piece| piece.len() > 1).map(GridPath::open).collect()
	}

	/// The generalized Hilbert curve through the points the rows reach, so the Symmetric shapes leave out the same points Rows Only does.
	fn hilbert_curve(&self) -> Vec<GridPath> {
		let full_rows = if self.grid_type == GridType::IsometricSymmetricMinusOne {
			(self.rows - 1).max(0)
		} else {
			self.rows
		};
		let height = |x: i64, y: i64| {
			if !self.isometric() || x % 2 == 0 {
				2 * y
			} else if self.grid_type == GridType::IsometricTiling {
				2 * y - 1
			} else {
				2 * y + 1
			}
		};

		let points = gilbert_curve(self.columns, full_rows).into_iter().filter_map(|(x, y)| self.at((x, height(x, y)))).collect();
		vec![GridPath::open(points)]
	}

	/// The corners around each cell between two neighboring columns, row by row: top left, top right, bottom right, then bottom left, where the grid may lack any of them.
	fn cell_corners(&self) -> impl Iterator<Item = (i64, [Option<usize>; 4])> + '_ {
		let corner = move |x: i64, row: i64| self.at((x, if self.isometric() && x % 2 == 1 { 2 * row - 1 } else { 2 * row }));

		(-1..=self.rows).flat_map(move |row| (0..self.columns - 1).map(move |x| (x, [corner(x, row), corner(x + 1, row), corner(x + 1, row + 1), corner(x, row + 1)])))
	}

	/// The two triangles of a cell whose corners all exist, split along the isometric grid's diagonal, or from top left to bottom right for rectangular cells.
	fn cell_triangles(&self, x: i64, [a, b, c, d]: [Option<usize>; 4]) -> Vec<Vec<usize>> {
		let triangles = if self.isometric() && x % 2 == 1 { [[a, b, d], [b, c, d]] } else { [[a, b, c], [a, c, d]] };
		triangles.into_iter().filter_map(|corners| corners.into_iter().collect::<Option<Vec<usize>>>()).collect()
	}

	/// Each cell's corners clockwise from its top left: squares, isometric rhombi (or triangles where the Symmetric shapes have no room for a rhombus), or Staggered's bricks and whole hexagons.
	fn cells(&self) -> Vec<Vec<usize>> {
		if self.stagger.is_some() {
			return self.staggered_faces();
		}

		self.cell_corners()
			.flat_map(|(x, corners)| match corners {
				[Some(a), Some(b), Some(c), Some(d)] => vec![vec![a, b, c, d]],
				_ => self.cell_triangles(x, corners),
			})
			.collect()
	}

	fn triangles(&self) -> Vec<Vec<usize>> {
		self.cell_corners().flat_map(|(x, corners)| self.cell_triangles(x, corners)).collect()
	}

	/// The faces between the edges, each running clockwise on screen: squares or triangles, or Staggered's bricks and whole hexagons.
	fn faces(&self) -> Vec<Vec<usize>> {
		if self.stagger.is_some() {
			self.staggered_faces()
		} else if self.isometric() {
			self.triangles()
		} else {
			self.cells()
		}
	}

	/// Staggered's bricks and whole hexagons, each running clockwise on screen from its top left (or, for hexagons, its top).
	fn staggered_faces(&self) -> Vec<Vec<usize>> {
		match self.stagger {
			None => Vec::new(),
			// Each brick runs along a row between two neighboring rungs and back along the row below
			Some(Stagger::Bricks) => {
				let mut bricks = Vec::new();
				for row in 0..self.rows - 1 {
					let height = 2 * row;
					let rungs: Vec<i64> = (0..self.columns).filter(|&x| self.keeps_rung((x, height))).collect();

					for pair in rungs.windows(2) {
						let top = (pair[0]..=pair[1]).filter_map(|x| self.at((x, height)));
						let bottom = (pair[0]..=pair[1]).rev().filter_map(|x| self.at((x, height + 2)));
						bricks.push(top.chain(bottom).collect());
					}
				}
				bricks
			}
			// Each hexagon surrounds a center whose six neighbors all exist
			Some(Stagger::Honeycomb { center }) => {
				const AROUND: [Coordinate; 6] = [(0, -2), (1, -1), (1, 1), (0, 2), (-1, 1), (-1, -1)];

				self.coordinates
					.iter()
					.filter(|&&coordinate| third(coordinate) == center)
					.filter_map(|&(x, height)| AROUND.iter().map(|&(step_x, step_height)| self.at((x + step_x, height + step_height))).collect::<Option<Vec<usize>>>())
					.collect()
			}
		}
	}

	/// Gives each polygon its own copies of its corners.
	fn separate_polygons(&self, polygons: Vec<Vec<usize>>) -> GridLayout {
		let mut positions = Vec::new();
		let paths = polygons
			.into_iter()
			.map(|polygon| {
				let start = positions.len();
				positions.extend(polygon.iter().map(|&corner| self.positions[corner]));
				GridPath::closed((start..positions.len()).collect())
			})
			.collect();

		GridLayout {
			positions,
			paths,
			kept_free_points: Vec::new(),
		}
	}

	/// Peels the outline off the remaining faces over and over, from the outside in, then joins any points left without faces into the fewest paths.
	/// Returns the rings and the lone points that no ring reaches.
	fn rings(&self) -> (Vec<GridPath>, Vec<usize>) {
		let faces = self.faces();
		let point_count = self.positions.len();

		// Faces run clockwise, so an edge running the other way belongs to the face across it
		let mut face_of_edge = FxHashMap::default();
		let mut faces_at_point = vec![Vec::new(); point_count];
		for (face_index, face) in faces.iter().enumerate() {
			face_of_edge.extend(face_edges(face).map(|edge| (edge, face_index)));
			for &point in face {
				faces_at_point[point].push(face_index);
			}
		}

		let mut face_alive = vec![true; faces.len()];
		let mut point_removed = vec![false; point_count];
		let mut rings = Vec::new();

		// The outline is made of the face edges with no face across them
		let mut outline: Vec<(usize, usize)> = face_of_edge.keys().copied().filter(|&(a, b)| !face_of_edge.contains_key(&(b, a))).collect();
		while !outline.is_empty() {
			let loops = self.chain_outline(&outline);

			// Removing the outline's points takes their faces with them, exposing the edges of the faces across
			let mut removed_faces = Vec::new();
			for &point in loops.iter().flatten() {
				point_removed[point] = true;
				for &face in &faces_at_point[point] {
					if std::mem::replace(&mut face_alive[face], false) {
						removed_faces.push(face);
					}
				}
			}

			outline = removed_faces
				.iter()
				.flat_map(|&face| face_edges(&faces[face]))
				.map(|(a, b)| (b, a))
				.filter(|edge| face_of_edge.get(edge).is_some_and(|&face| face_alive[face]))
				.collect();
			outline.sort_unstable();
			outline.dedup();

			rings.extend(loops.into_iter().map(GridPath::closed));
		}

		// Whatever is left without faces forms the fewest paths through its edges, or lone points
		let leftover_edges: Vec<(usize, usize)> = self.edges().into_iter().filter(|&(a, b)| !point_removed[a] && !point_removed[b]).collect();
		let mut reached = point_removed;
		for &(a, b) in &leftover_edges {
			(reached[a], reached[b]) = (true, true);
		}
		rings.extend(fewest_trails(&self.positions, &leftover_edges));

		(rings, (0..point_count).filter(|&point| !reached[point]).collect())
	}

	/// Chains outline edges into closed loops, each starting from its top-left point and running clockwise on screen.
	fn chain_outline(&self, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
		let mut edges = edges.to_vec();
		edges.sort_unstable();
		let mut used = vec![false; edges.len()];
		let leaving = |point: usize| edges.partition_point(|&(start, _)| start < point)..edges.partition_point(|&(start, _)| start <= point);

		// How far an edge turns right from the direction arriving at its start
		let turn = |arriving: DVec2, edge: usize| {
			let (start, end) = edges[edge];
			let leaving = self.positions[end] - self.positions[start];
			arriving.perp_dot(leaving).atan2(arriving.dot(leaving))
		};

		// Trying each loop's start from the top-left-most point first makes that point the top left of its loop
		let mut starts: Vec<usize> = edges.iter().map(|&(start, _)| start).collect();
		starts.dedup();
		starts.sort_by(|&a, &b| {
			let (a, b) = (self.positions[a], self.positions[b]);
			a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x))
		});

		let mut loops = Vec::new();
		for start in starts {
			while let Some(mut edge) = leaving(start).find(|&edge| !used[edge]) {
				let mut points = vec![start];
				loop {
					used[edge] = true;
					let (from, to) = edges[edge];
					if to == start {
						break;
					}
					points.push(to);

					// Where the outline pinches through a point, the sharpest right turn stays with the same faces
					let arriving = self.positions[to] - self.positions[from];
					let Some(next) = leaving(to).filter(|&next| !used[next]).max_by(|&a, &b| turn(arriving, a).total_cmp(&turn(arriving, b))) else {
						break;
					};
					edge = next;
				}

				// Holes come out counterclockwise, so they turn around to match
				if signed_area(&self.positions, &points) < 0. {
					points[1..].reverse();
				}
				loops.push(points);
			}
		}

		loops
	}

	/// One route winding inward through the rings, each lap starting beside where the last one ended and ending beside the next, split wherever no edge links two laps.
	fn spiral(&self) -> Vec<GridPath> {
		let (rings, lone_points) = self.rings();
		let laps: Vec<GridPath> = rings.into_iter().chain(lone_points.into_iter().map(|point| GridPath::open(vec![point]))).collect();

		let mut lap_of_point = vec![usize::MAX; self.positions.len()];
		for (index, lap) in laps.iter().enumerate() {
			for &point in &lap.points {
				lap_of_point[point] = index;
			}
		}

		let mut paths = Vec::new();
		let mut route: Vec<usize> = Vec::new();
		let mut visited = vec![false; self.positions.len()];
		for (index, lap) in laps.iter().enumerate() {
			let end = route.last().copied();
			let follows_end = |point: usize| end.is_none_or(|end| self.are_neighbors(end, point));
			let reaches_next = |point: usize| index + 1 == laps.len() || self.neighbors(point).any(|neighbor| lap_of_point[neighbor] == index + 1);

			// A closed lap can start anywhere, going all the way around, and an open one from either end
			let count = lap.points.len();
			let way_count = if lap.closed { count } else { count.min(2) };
			let first_and_last = |way: usize| match (lap.closed, way) {
				(true, start) => (lap.points[start], lap.points[(start + count - 1) % count]),
				(false, 0) => (lap.points[0], lap.points[count - 1]),
				(false, _) => (lap.points[count - 1], lap.points[0]),
			};
			let way = (0..way_count)
				.find(|&way| {
					let (first, last) = first_and_last(way);
					follows_end(first) && reaches_next(last)
				})
				.or_else(|| (0..way_count).find(|&way| follows_end(first_and_last(way).0)));

			if way.is_none() {
				paths.push(std::mem::take(&mut route));
			}
			let lap_route: Vec<usize> = match (lap.closed, way.unwrap_or(0)) {
				(true, start) => lap.points[start..].iter().chain(&lap.points[..start]).copied().collect(),
				(false, 0) => lap.points.clone(),
				(false, _) => lap.points.iter().rev().copied().collect(),
			};

			// Leftover paths can cross where they branch, so the route skips points it already passed
			route.extend(lap_route.into_iter().filter(|&point| !std::mem::replace(&mut visited[point], true)));
		}
		paths.push(route);
		let mut paths: Vec<GridPath> = paths.into_iter().flat_map(|path| self.split_where_unconnected(path)).collect();

		// A point left off the route slips in between two of its neighbors that are consecutive on it
		let mut on_route = vec![false; self.positions.len()];
		for &point in paths.iter().flat_map(|path| &path.points) {
			on_route[point] = true;
		}
		for point in (0..self.positions.len()).filter(|&point| !on_route[point]) {
			for path in &mut paths {
				if let Some(index) = path.points.windows(2).position(|pair| self.are_neighbors(pair[0], point) && self.are_neighbors(point, pair[1])) {
					path.points.insert(index + 1, point);
					break;
				}
			}
		}

		paths
	}

	/// A closed route through every point, built as a comb: along the first line, back and forth through the other lines below their first points,
	/// then back along those first points. Rectangular grids with an odd number of both columns and rows leave out their bottom-right corner.
	fn loop_route(&self) -> Option<Vec<usize>> {
		let (columns, rows) = (self.columns as usize, self.rows as usize);

		let route = if !self.isometric() {
			let point = |x: usize, y: usize| self.column_points[x][y];

			if columns < 2 || rows < 2 {
				return None;
			} else if columns % 2 == 0 {
				comb(&self.column_points)
			} else if rows % 2 == 0 {
				comb(&(0..rows).map(|y| (0..columns).map(|x| point(x, y)).collect::<Vec<_>>()).collect::<Vec<_>>())
			} else {
				// Along the top row and back and forth through the rows below, then up and down through the last two rows from right to left, and back up the first column
				let mut route: Vec<usize> = (0..columns).map(|x| point(x, 0)).collect();
				for y in 1..rows - 2 {
					if y % 2 == 1 {
						route.extend((1..columns).rev().map(|x| point(x, y)));
					} else {
						route.extend((1..columns).map(|x| point(x, y)));
					}
				}
				route.push(point(columns - 1, rows - 2));
				for x in (1..columns - 1).rev() {
					let (upper, lower) = (point(x, rows - 2), point(x, rows - 1));
					route.extend(if (columns - 2 - x) % 2 == 0 { [upper, lower] } else { [lower, upper] });
				}
				route.extend((1..rows).rev().map(|y| point(0, y)));
				route
			}
		} else {
			if columns < 2 || self.column_points.iter().any(|column| column.len() < 2) {
				return None;
			}

			if columns % 2 == 0 {
				comb(&self.column_points)
			} else {
				let mut route = self.column_points[0].clone();
				for x in 1..columns - 2 {
					let below_top = &self.column_points[x][1..];
					if x % 2 == 1 {
						route.extend(below_top.iter().rev());
					} else {
						route.extend(below_top);
					}
				}

				// Below their top points, the last two columns climb together as one zigzag, starting from the bottom of the nearer column
				let mut strip: Vec<usize> = self.column_points[columns - 2][1..].iter().chain(&self.column_points[columns - 1][1..]).copied().collect();
				strip.sort_by_key(|&point| std::cmp::Reverse(self.coordinates[point].1));
				if strip.len() >= 2 && self.coordinates[strip[0]].0 == (columns - 1) as i64 {
					strip.swap(0, 1);
				}
				route.extend(strip);

				route.extend((1..columns).rev().map(|x| self.column_points[x][0]));
				route
			}
		};

		// Only a route that follows an edge at every step, including the one closing it, counts
		let follows_edges = route.len() > 2 && route.iter().zip(route.iter().cycle().skip(1)).all(|(&a, &b)| self.are_neighbors(a, b));
		follows_edges.then_some(route)
	}

	/// One path per connected group of edges, drawing every edge by linking the fewest-paths trails along routes drawn twice.
	fn single_path(&self) -> Vec<GridPath> {
		let edges = self.edges();

		let mut adjacency = vec![Vec::new(); self.positions.len()];
		for &(a, b) in &edges {
			adjacency[a].push(b);
			adjacency[b].push(a);
		}

		let mut doubled = edges;
		let mut search = Search::new(&adjacency);
		for (start, end) in pair_odd_points(&adjacency) {
			search.run(start, usize::MAX, |point| point == end);
			doubled.extend(search.route_to(end).windows(2).map(|pair| (pair[0], pair[1])));
		}

		fewest_trails(&self.positions, &doubled)
	}
}

/// Which of three interleaved sets of honeycomb hexagon centers an isometric point belongs to. No two points in the same set are neighbors.
fn third((x, height): Coordinate) -> i64 {
	((3 * x - height) / 2).rem_euclid(3)
}

/// The edges around a face, each from one corner to the next.
fn face_edges(face: &[usize]) -> impl Iterator<Item = (usize, usize)> + '_ {
	face.iter().zip(face.iter().cycle().skip(1)).map(|(&a, &b)| (a, b))
}

/// Twice the area a loop encloses, positive when it runs clockwise on screen.
fn signed_area(positions: &[DVec2], points: &[usize]) -> f64 {
	face_edges(points).map(|(a, b)| positions[a].perp_dot(positions[b])).sum()
}

/// Along the first line, back and forth through each later line below its first point, then back along those first points to close the loop.
/// Needs an even number of lines.
fn comb(lines: &[Vec<usize>]) -> Vec<usize> {
	let mut route = lines[0].clone();
	for (index, line) in lines.iter().enumerate().skip(1) {
		if index % 2 == 1 {
			route.extend(line[1..].iter().rev());
		} else {
			route.extend(&line[1..]);
		}
	}
	route.extend(lines[1..].iter().rev().map(|line| line[0]));

	route
}

/// The generalized Hilbert curve (the Gilbert curve) through every cell of a rectangle, matching the Hilbert curve on power-of-two squares.
/// It takes one diagonal step when the long side is odd and the short side even, where no route along the edges can link its two corners.
fn gilbert_curve(width: i64, height: i64) -> Vec<(i64, i64)> {
	fn generate(order: &mut Vec<(i64, i64)>, (x, y): (i64, i64), (ax, ay): (i64, i64), (bx, by): (i64, i64)) {
		let (width, height) = ((ax + ay).abs(), (bx + by).abs());
		let (step_ax, step_ay, step_bx, step_by) = (ax.signum(), ay.signum(), bx.signum(), by.signum());

		// A single row or column is walked straight through
		if height == 1 || width == 1 {
			let ((step_x, step_y), count) = if height == 1 { ((step_ax, step_ay), width) } else { ((step_bx, step_by), height) };
			order.extend((0..count).map(|i| (x + step_x * i, y + step_y * i)));
			return;
		}

		let (mut ax_half, mut ay_half, mut bx_half, mut by_half) = (ax.div_euclid(2), ay.div_euclid(2), bx.div_euclid(2), by.div_euclid(2));
		let (width_half, height_half) = ((ax_half + ay_half).abs(), (bx_half + by_half).abs());

		if 2 * width > 3 * height {
			// Long rectangles split in two along their length
			if width_half % 2 == 1 && width > 2 {
				(ax_half, ay_half) = (ax_half + step_ax, ay_half + step_ay);
			}
			generate(order, (x, y), (ax_half, ay_half), (bx, by));
			generate(order, (x + ax_half, y + ay_half), (ax - ax_half, ay - ay_half), (bx, by));
		} else {
			// Others split into the Hilbert curve's up, across, and down thirds
			if height_half % 2 == 1 && height > 2 {
				(bx_half, by_half) = (bx_half + step_bx, by_half + step_by);
			}
			generate(order, (x, y), (bx_half, by_half), (ax_half, ay_half));
			generate(order, (x + bx_half, y + by_half), (ax, ay), (bx - bx_half, by - by_half));
			generate(
				order,
				(x + (ax - step_ax) + (bx_half - step_bx), y + (ay - step_ay) + (by_half - step_by)),
				(-bx_half, -by_half),
				(-(ax - ax_half), -(ay - ay_half)),
			);
		}
	}

	let mut order = Vec::new();
	if width > 0 && height > 0 {
		if width >= height {
			generate(&mut order, (0, 0), (width, 0), (0, height));
		} else {
			generate(&mut order, (0, 0), (0, height), (width, 0));
		}
	}

	order
}

/// Joins edges into the fewest trails that draw each edge once, each continuing as straight as possible through the points it passes.
/// A connected group of edges becomes one closed trail if every point has an even number of edges, or else one open trail per pair of points with an odd number.
/// Runs in O(edges) for the bounded number of edges at each point.
fn fewest_trails(positions: &[DVec2], edges: &[(usize, usize)]) -> Vec<GridPath> {
	const NONE: usize = usize::MAX;

	// Each edge has two ends, numbered `2 * edge` at its start and `2 * edge + 1` at its end
	let point_of = |end: usize| if end.is_multiple_of(2) { edges[end / 2].0 } else { edges[end / 2].1 };
	let mut ends_at = vec![Vec::new(); positions.len()];
	for end in 0..edges.len() * 2 {
		ends_at[point_of(end)].push(end);
	}

	// At each point, partner up the edge ends that continue each other most straightly, leaving one unpartnered where the count is odd
	let mut partner = vec![NONE; edges.len() * 2];
	for (point, ends) in ends_at.iter().enumerate() {
		let direction = |end: usize| (positions[point_of(end ^ 1)] - positions[point]).normalize_or_zero();

		let mut unpartnered = ends.clone();
		while unpartnered.len() >= 2 {
			let mut straightest = (f64::INFINITY, 0, 1);
			for i in 0..unpartnered.len() {
				for j in i + 1..unpartnered.len() {
					let alignment = direction(unpartnered[i]).dot(direction(unpartnered[j]));
					if alignment < straightest.0 - 1e-9 {
						straightest = (alignment, i, j);
					}
				}
			}

			let (_, i, j) = straightest;
			let (a, b) = (unpartnered.remove(j), unpartnered.remove(i));
			partner[a] = b;
			partner[b] = a;
		}
	}

	// Label the trails and loops these partners make, noting which are closed loops
	let mut group = vec![NONE; edges.len()];
	let mut closed = Vec::new();
	for seed in 0..edges.len() {
		if group[seed] != NONE {
			continue;
		}

		let label = closed.len();
		let mut is_closed = true;
		group[seed] = label;
		let mut stack = vec![seed];
		while let Some(edge) = stack.pop() {
			for end in [2 * edge, 2 * edge + 1] {
				match partner[end] {
					NONE => is_closed = false,
					other if group[other / 2] == NONE => {
						group[other / 2] = label;
						stack.push(other / 2);
					}
					_ => {}
				}
			}
		}
		closed.push(is_closed);
	}

	// Splice each loop into a trail or loop it shares a point with by swapping partners there, merging the two
	let mut merged_into: Vec<usize> = (0..closed.len()).collect();
	let root = |merged_into: &mut Vec<usize>, mut label: usize| {
		while merged_into[label] != label {
			merged_into[label] = merged_into[merged_into[label]];
			label = merged_into[label];
		}
		label
	};
	for ends in &ends_at {
		'merging: loop {
			// Each partnered pair of ends passing through, and the unpartnered end if there is one
			let slots: Vec<(usize, usize)> = ends.iter().filter(|&&end| partner[end] == NONE || end < partner[end]).map(|&end| (end, partner[end])).collect();

			for i in 0..slots.len() {
				for j in i + 1..slots.len() {
					let (first, second) = (root(&mut merged_into, group[slots[i].0 / 2]), root(&mut merged_into, group[slots[j].0 / 2]));
					if first == second || !(closed[first] || closed[second]) {
						continue;
					}

					let ((a, a_partner), (b, b_partner)) = (slots[i], slots[j]);
					partner[a] = b;
					partner[b] = a;
					match (a_partner, b_partner) {
						(NONE, _) => partner[b_partner] = NONE,
						(_, NONE) => partner[a_partner] = NONE,
						_ => {
							partner[a_partner] = b_partner;
							partner[b_partner] = a_partner;
						}
					}

					merged_into[second] = first;
					closed[first] &= closed[second];
					continue 'merging;
				}
			}

			break;
		}
	}

	// Follow each trail from its first unpartnered end, then each loop left over
	let trace = |start: usize, visited: &mut Vec<bool>| {
		let mut points = vec![point_of(start)];
		let mut end = start;
		loop {
			visited[end / 2] = true;
			points.push(point_of(end ^ 1));

			match partner[end ^ 1] {
				NONE => break,
				next if next == start => break,
				next => end = next,
			}
		}
		points
	};

	let mut visited = vec![false; edges.len()];
	let mut trails = Vec::new();
	for end in ends_at.iter().flatten().copied() {
		if partner[end] == NONE && !visited[end / 2] {
			trails.push(GridPath::open(trace(end, &mut visited)));
		}
	}
	for edge in 0..edges.len() {
		if !visited[edge] {
			let mut points = trace(2 * edge, &mut visited);
			points.pop();
			trails.push(GridPath::closed(points));
		}
	}

	trails
}

/// Pairs up the points with an odd number of edges, closest first, searching ever farther for those left over until each connected group has two or none left.
/// A group left with none gives back its longest pair, whose points become the ends of its path.
fn pair_odd_points(adjacency: &[Vec<usize>]) -> Vec<(usize, usize)> {
	let point_count = adjacency.len();
	let odd: Vec<bool> = adjacency.iter().map(|neighbors| neighbors.len() % 2 == 1).collect();

	// Label each connected group of points
	let mut group = vec![usize::MAX; point_count];
	let mut group_count = 0;
	for seed in 0..point_count {
		if group[seed] != usize::MAX {
			continue;
		}
		group[seed] = group_count;
		let mut stack = vec![seed];
		while let Some(point) = stack.pop() {
			for &neighbor in &adjacency[point] {
				if group[neighbor] == usize::MAX {
					group[neighbor] = group_count;
					stack.push(neighbor);
				}
			}
		}
		group_count += 1;
	}

	let unpaired_per_group = |paired: &[bool]| {
		let mut counts = vec![0; group_count];
		for point in (0..point_count).filter(|&point| odd[point] && !paired[point]) {
			counts[group[point]] += 1;
		}
		counts
	};

	let mut paired = vec![false; point_count];
	let mut pairs: Vec<(usize, usize, usize)> = Vec::new();
	let mut search = Search::new(adjacency);
	let mut radius = 2;
	loop {
		// Only groups with more than two points left unpaired keep searching
		let unpaired = unpaired_per_group(&paired);
		let searching: Vec<usize> = (0..point_count).filter(|&point| odd[point] && !paired[point] && unpaired[group[point]] > 2).collect();
		if searching.is_empty() {
			break;
		}

		let mut candidates = Vec::new();
		for &start in &searching {
			search.run(start, radius, |_| false);
			candidates.extend(
				search
					.visited
					.iter()
					.filter(|&&point| point > start && odd[point] && !paired[point])
					.map(|&point| (search.distance[point], start, point)),
			);
		}

		candidates.sort_unstable();
		for (distance, a, b) in candidates {
			if !paired[a] && !paired[b] {
				(paired[a], paired[b]) = (true, true);
				pairs.push((distance, a, b));
			}
		}

		if radius >= point_count {
			break;
		}
		radius *= 2;
	}

	// Groups with nothing left unpaired give back their longest pair, so their paths stay open
	let unpaired = unpaired_per_group(&paired);
	let mut longest: Vec<Option<usize>> = vec![None; group_count];
	for (index, &(distance, a, _)) in pairs.iter().enumerate() {
		if unpaired[group[a]] == 0 && longest[group[a]].is_none_or(|longest| distance > pairs[longest].0) {
			longest[group[a]] = Some(index);
		}
	}

	pairs
		.iter()
		.enumerate()
		.filter(|&(index, &(_, a, _))| longest[group[a]] != Some(index))
		.map(|(_, &(_, a, b))| (a, b))
		.collect()
}

/// A breadth-first search that reuses its buffers, so each search only costs the area it covers.
struct Search<'a> {
	adjacency: &'a [Vec<usize>],
	distance: Vec<usize>,
	previous: Vec<usize>,
	visited: Vec<usize>,
	queue: VecDeque<usize>,
}

impl<'a> Search<'a> {
	fn new(adjacency: &'a [Vec<usize>]) -> Self {
		Self {
			adjacency,
			distance: vec![usize::MAX; adjacency.len()],
			previous: vec![usize::MAX; adjacency.len()],
			visited: Vec::new(),
			queue: VecDeque::new(),
		}
	}

	/// Visits points out to `radius` edges from `start`, or until reaching a point `stop` accepts.
	fn run(&mut self, start: usize, radius: usize, stop: impl Fn(usize) -> bool) {
		for &point in &self.visited {
			self.distance[point] = usize::MAX;
		}
		self.visited.clear();
		self.queue.clear();

		self.distance[start] = 0;
		self.visited.push(start);
		self.queue.push_back(start);
		while let Some(point) = self.queue.pop_front() {
			if point != start && stop(point) {
				return;
			}
			if self.distance[point] >= radius {
				continue;
			}

			for &neighbor in &self.adjacency[point] {
				if self.distance[neighbor] == usize::MAX {
					self.distance[neighbor] = self.distance[point] + 1;
					self.previous[neighbor] = point;
					self.visited.push(neighbor);
					self.queue.push_back(neighbor);
				}
			}
		}
	}

	/// The route the last search took from its start to a point it reached.
	fn route_to(&self, end: usize) -> Vec<usize> {
		if self.distance[end] == usize::MAX {
			return Vec::new();
		}

		let mut route = vec![end];
		let mut point = end;
		while self.distance[point] > 0 {
			point = self.previous[point];
			route.push(point);
		}
		route.reverse();

		route
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use vector_types::vector::paths_from_item_attributes;

	const GRID_TYPES: [GridType; 4] = [
		GridType::Rectangular,
		GridType::IsometricTiling,
		GridType::IsometricSymmetricPlusOne,
		GridType::IsometricSymmetricMinusOne,
	];
	const TOPOLOGIES: [GridTopology; 14] = [
		GridTopology::Lines,
		GridTopology::Edges,
		GridTopology::FewestPaths,
		GridTopology::SinglePath,
		GridTopology::RowsOnly,
		GridTopology::ColumnsOnly,
		GridTopology::PointsOnly,
		GridTopology::Serpentine,
		GridTopology::HilbertCurve,
		GridTopology::Spiral,
		GridTopology::Loop,
		GridTopology::Rings,
		GridTopology::Cells,
		GridTopology::Triangles,
	];

	/// Every number of columns and rows from 2 to 9, covering each mix of odd and even.
	fn sizes() -> impl Iterator<Item = (u32, u32)> {
		(2..=9).flat_map(|columns| (2..=9).map(move |rows| (columns, rows)))
	}

	/// A lattice with rows one unit apart and isometric columns spaced for 30° angles.
	fn lattice(grid_type: GridType, columns: u32, rows: u32, staggered: bool) -> Lattice {
		let isometric = grid_type.is_isometric();
		let column_width = if isometric { 3_f64.sqrt() / 2. } else { 1. };

		Lattice::new(grid_type, columns, rows, staggered, |x, y| {
			let lift = if isometric && x % 2 == 1 { 0.5 } else { 0. };
			DVec2::new(x as f64 * column_width, y as f64 - lift)
		})
	}

	/// Runs a check on each grid type and size, both with and without Staggered where the topology supports it.
	fn each_lattice(topology: GridTopology, mut check: impl FnMut(&Lattice, &str)) {
		for grid_type in GRID_TYPES {
			for (columns, rows) in sizes() {
				for staggered in [false, true].into_iter().filter(|&staggered| !staggered || topology.supports_staggered()) {
					check(
						&lattice(grid_type, columns, rows, staggered),
						&format!("{topology:?} on {grid_type:?} {columns}x{rows}, staggered: {staggered}"),
					);
				}
			}
		}
	}

	fn sorted_edges(lattice: &Lattice) -> Vec<(usize, usize)> {
		let mut edges: Vec<(usize, usize)> = lattice.edges().into_iter().map(|(a, b)| (a.min(b), a.max(b))).collect();
		edges.sort_unstable();
		edges
	}

	/// Each segment the paths draw, from its lower point index to its higher, sorted.
	fn drawn_segments(layout: &GridLayout) -> Vec<(usize, usize)> {
		let mut segments: Vec<(usize, usize)> = layout.paths.iter().flat_map(path_steps).map(|(a, b)| (a.min(b), a.max(b))).collect();
		segments.sort_unstable();
		segments
	}

	/// Each step along a path, including the one closing it.
	fn path_steps(path: &GridPath) -> impl Iterator<Item = (usize, usize)> + '_ {
		let closing = (path.closed && path.points.len() > 2).then(|| (path.points[path.points.len() - 1], path.points[0]));
		path.points.windows(2).map(|pair| (pair[0], pair[1])).chain(closing)
	}

	/// The number of points with an odd number of edges in each connected group of edges.
	fn odd_points_per_group(lattice: &Lattice) -> Vec<usize> {
		let mut adjacency = vec![Vec::new(); lattice.positions.len()];
		for (a, b) in lattice.edges() {
			adjacency[a].push(b);
			adjacency[b].push(a);
		}

		let mut grouped = vec![false; adjacency.len()];
		let mut odd_counts = Vec::new();
		for seed in 0..adjacency.len() {
			if grouped[seed] || adjacency[seed].is_empty() {
				continue;
			}

			let mut odd_count = 0;
			grouped[seed] = true;
			let mut stack = vec![seed];
			while let Some(point) = stack.pop() {
				odd_count += adjacency[point].len() % 2;
				for &neighbor in &adjacency[point] {
					if !std::mem::replace(&mut grouped[neighbor], true) {
						stack.push(neighbor);
					}
				}
			}
			odd_counts.push(odd_count);
		}

		odd_counts
	}

	/// Checks that the paths visit each point at most once and step only along edges, returning how many points they visit.
	fn count_visits_along_edges(lattice: &Lattice, layout: &GridLayout, case: &str) -> usize {
		let mut visits = vec![0; lattice.positions.len()];
		for path in &layout.paths {
			for &point in &path.points {
				visits[point] += 1;
			}
			for (a, b) in path_steps(path) {
				assert!(lattice.are_neighbors(a, b), "{case}: the step from point {a} to {b} follows no edge");
			}
		}

		assert!(visits.iter().all(|&count| count <= 1), "{case}: a point is visited more than once");
		visits.iter().sum()
	}

	/// The edges in the order the grid stored them before it had topologies, from each point back to its neighbors stored earlier.
	fn legacy_edges(isometric: bool, columns: usize, rows: usize) -> Vec<(usize, usize)> {
		let mut edges = Vec::new();

		for y in 0..rows {
			for x in 0..columns {
				let current = y * columns + x;
				let mut earlier = vec![(x > 0).then(|| current - 1), current.checked_sub(columns)];
				if isometric && x % 2 == 1 {
					earlier.push(current.checked_sub(columns - 1).filter(|_| x + 1 < columns));
					earlier.push(current.checked_sub(columns + 1));
				}
				edges.extend(earlier.into_iter().flatten().map(|neighbor| (neighbor, current)));
			}
		}

		edges
	}

	#[test]
	fn small_grids_lay_out_without_panicking() {
		for grid_type in GRID_TYPES {
			for columns in 0..=3 {
				for rows in 0..=3 {
					for staggered in [false, true] {
						for topology in TOPOLOGIES {
							lattice(grid_type, columns, rows, staggered && topology.supports_staggered()).layout(topology).into_item();
						}
					}
				}
			}
		}
	}

	#[test]
	fn only_points_only_keeps_points_no_path_runs_through() {
		for topology in TOPOLOGIES {
			each_lattice(topology, |lattice, case| {
				let layout = lattice.layout(topology);
				let kept_free_points = layout.kept_free_points.iter().filter(|&&kept| kept).count();
				let item = layout.into_item();

				let mut used = vec![false; item.element().0.len()];
				for path in paths_from_item_attributes(&item).into_iter().flat_map(|paths| paths.iter_element_values()) {
					for reference in path.references() {
						used[reference as usize] = true;
					}
				}

				if topology == GridTopology::PointsOnly {
					assert_eq!(used.len(), kept_free_points, "{case}");
				} else {
					assert!(used.iter().all(|&used| used), "{case} has {} free points", used.iter().filter(|&&used| !used).count());
				}
			});
		}
	}

	#[test]
	fn a_one_by_one_grid_is_empty_except_with_points_only() {
		for grid_type in GRID_TYPES {
			for topology in TOPOLOGIES {
				let point_count = lattice(grid_type, 1, 1, false).layout(topology).into_item().element().0.len();
				assert_eq!(point_count, usize::from(topology == GridTopology::PointsOnly), "{topology:?} on {grid_type:?}");
			}
		}
	}

	#[test]
	fn leaving_out_free_points_keeps_the_ids_of_the_rest() {
		// Staggered Lines drops the honeycomb's centers and Staggered Points Only keeps just them, but each point either keeps has its unstaggered ID
		for topology in [GridTopology::Lines, GridTopology::PointsOnly] {
			for grid_type in GRID_TYPES.into_iter().filter(|grid_type| grid_type.is_isometric()) {
				let ids_by_position = |staggered: bool| {
					let item = lattice(grid_type, 6, 5, staggered).layout(topology).into_item();
					let pool = &item.element().0;
					let ids = pool.iter_attribute_values::<PointId>(ATTR_ID).map(|ids| ids.as_slice().to_vec()).unwrap_or_default();
					pool.iter_element_values().copied().zip(ids).collect::<Vec<(DVec2, PointId)>>()
				};

				let (staggered, unstaggered) = (ids_by_position(true), ids_by_position(false));
				assert!(staggered.len() < unstaggered.len(), "{topology:?} on {grid_type:?}");
				assert!(staggered.iter().all(|kept| unstaggered.contains(kept)), "{topology:?} on {grid_type:?}");
			}
		}
	}

	#[test]
	fn edges_run_back_to_each_neighbor_stored_earlier() {
		each_lattice(GridTopology::Edges, |lattice, case| {
			let mut from_neighbors: Vec<(usize, usize)> = (0..lattice.positions.len())
				.flat_map(|point| lattice.neighbors(point).filter(move |&neighbor| neighbor < point).map(move |neighbor| (neighbor, point)))
				.collect();
			from_neighbors.sort_unstable();

			assert!(lattice.edges().iter().all(|&(earlier, later)| earlier < later), "{case}");
			assert_eq!(sorted_edges(lattice), from_neighbors, "{case}");
		});
	}

	#[test]
	fn edges_and_cells_keep_the_legacy_order() {
		for (grid_type, isometric) in [(GridType::Rectangular, false), (GridType::IsometricTiling, true)] {
			for (columns, rows) in sizes() {
				let case = format!("{grid_type:?} {columns}x{rows}");
				let lattice = lattice(grid_type, columns, rows, false);
				assert_eq!(lattice.edges(), legacy_edges(isometric, columns as usize, rows as usize), "{case}");

				let position = |x: u32, y: u32| lattice.positions[(y * columns + x) as usize];
				let legacy_cells: Vec<DVec2> = (0..rows - 1)
					.flat_map(|y| (0..columns - 1).flat_map(move |x| [position(x, y), position(x + 1, y), position(x + 1, y + 1), position(x, y + 1)]))
					.collect();
				assert_eq!(lattice.layout(GridTopology::Cells).positions, legacy_cells, "{case}");
			}
		}
	}

	#[test]
	fn lines_edges_and_fewest_paths_draw_each_edge_once() {
		for topology in [GridTopology::Lines, GridTopology::Edges, GridTopology::FewestPaths] {
			each_lattice(topology, |lattice, case| assert_eq!(drawn_segments(&lattice.layout(topology)), sorted_edges(lattice), "{case}"));
		}
	}

	#[test]
	fn fewest_paths_end_in_pairs_of_odd_points() {
		each_lattice(GridTopology::FewestPaths, |lattice, case| {
			let expected: usize = odd_points_per_group(lattice).iter().map(|&odd_count| (odd_count / 2).max(1)).sum();
			assert_eq!(lattice.layout(GridTopology::FewestPaths).paths.len(), expected, "{case}");
		});
	}

	#[test]
	fn single_path_draws_every_edge_in_one_path_per_group() {
		each_lattice(GridTopology::SinglePath, |lattice, case| {
			let layout = lattice.layout(GridTopology::SinglePath);
			let edges = sorted_edges(lattice);

			let mut drawn = drawn_segments(&layout);
			assert!(drawn.iter().all(|segment| edges.binary_search(segment).is_ok()), "{case}: a segment follows no edge");
			drawn.dedup();
			assert_eq!(drawn, edges, "{case}");

			assert_eq!(layout.paths.len(), odd_points_per_group(lattice).len(), "{case}");
		});
	}

	#[test]
	fn routes_visit_each_point_once_along_edges() {
		for topology in [GridTopology::RowsOnly, GridTopology::ColumnsOnly, GridTopology::Serpentine, GridTopology::Spiral] {
			each_lattice(topology, |lattice, case| {
				let visits = count_visits_along_edges(lattice, &lattice.layout(topology), case);

				let reaches_every_point = match topology {
					GridTopology::RowsOnly => lattice.grid_type == GridType::Rectangular || (lattice.grid_type == GridType::IsometricTiling && lattice.stagger.is_none()),
					GridTopology::ColumnsOnly => lattice.column_points.iter().all(|column| column.len() > 1),
					_ => true,
				};
				if reaches_every_point {
					assert_eq!(visits, lattice.positions.len(), "{case}");
				}
			});
		}
	}

	#[test]
	fn hilbert_curve_visits_each_point_once() {
		each_lattice(GridTopology::HilbertCurve, |lattice, case| {
			let layout = lattice.layout(GridTopology::HilbertCurve);
			let points = &layout.paths[0].points;

			let mut distinct = points.clone();
			distinct.sort_unstable();
			distinct.dedup();
			assert_eq!(distinct.len(), points.len(), "{case}: a point is visited more than once");

			let leaps = points.windows(2).filter(|pair| !lattice.are_neighbors(pair[0], pair[1])).count();
			assert!(leaps <= 1, "{case}: {leaps} steps follow no edge");

			if matches!(lattice.grid_type, GridType::Rectangular | GridType::IsometricTiling) {
				assert_eq!(points.len(), lattice.positions.len(), "{case}");
			}
		});
	}

	#[test]
	fn loop_closes_through_every_point() {
		each_lattice(GridTopology::Loop, |lattice, case| {
			if lattice.column_points.iter().any(|column| column.len() < 2) {
				return;
			}

			let layout = lattice.layout(GridTopology::Loop);
			assert!(layout.paths.len() == 1 && layout.paths[0].closed, "{case}: no loop");

			// Rectangular grids with an odd number of both columns and rows leave out a corner
			let odd_by_odd = lattice.grid_type == GridType::Rectangular && lattice.columns % 2 == 1 && lattice.rows % 2 == 1;
			let expected = lattice.positions.len() - usize::from(odd_by_odd);
			assert_eq!(count_visits_along_edges(lattice, &layout, case), expected, "{case}");
		});
	}

	#[test]
	fn closed_shapes_run_clockwise() {
		for topology in [GridTopology::Rings, GridTopology::Cells, GridTopology::Triangles] {
			each_lattice(topology, |lattice, case| {
				let layout = lattice.layout(topology);
				for path in layout.paths.iter().filter(|path| path.closed) {
					assert!(signed_area(&layout.positions, &path.points) > 0., "{case}: a shape runs counterclockwise");
				}

				if topology == GridTopology::Rings {
					let edges = sorted_edges(lattice);
					assert!(drawn_segments(&layout).iter().all(|segment| edges.binary_search(segment).is_ok()), "{case}: a segment follows no edge");
				}
			});
		}
	}

	#[test]
	fn staggered_points_keep_at_most_three_edges() {
		for grid_type in GRID_TYPES {
			for (columns, rows) in sizes() {
				let lattice = lattice(grid_type, columns, rows, true);
				for point in 0..lattice.positions.len() {
					let count = lattice.neighbors(point).count();
					assert!(count <= 3, "{grid_type:?} {columns}x{rows}: point {point} keeps {count} edges");
				}
			}
		}
	}

	#[test]
	fn staggered_isometric_grids_surround_their_hexagon_centers() {
		let lattice = lattice(GridType::IsometricTiling, 7, 6, true);
		let Some(Stagger::Honeycomb { center }) = lattice.stagger else {
			panic!("Staggered isometric grids should be honeycombs")
		};

		let centers: Vec<DVec2> = (0..lattice.positions.len())
			.filter(|&point| third(lattice.coordinates[point]) == center)
			.map(|point| lattice.positions[point])
			.collect();
		assert!(!centers.is_empty());
		let points_only = lattice.layout(GridTopology::PointsOnly).into_item();
		assert_eq!(points_only.element().0.iter_element_values().copied().collect::<Vec<DVec2>>(), centers);

		let cells = lattice.layout(GridTopology::Cells);
		assert!(!cells.paths.is_empty());
		assert!(cells.paths.iter().all(|path| path.points.len() == 6), "Each cell should be a hexagon");
	}
}
