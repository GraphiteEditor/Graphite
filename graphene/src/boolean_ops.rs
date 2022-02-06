use crate::{
	consts::F64PRECISION,
	intersection::{intersections, line_curve_intersections, valid_t, Intersect, Origin},
	layers::{simple_shape::Shape, style::PathStyle},
};
use kurbo::{BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveArea, ParamCurveExtrema, PathEl, PathSeg, Point, QuadBez, Rect};
use serde::{Deserialize, Serialize};
use std::{
	fmt::{self, Debug, Formatter},
	num::IntErrorKind,
	ops::Deref,
	path,
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub enum BooleanOperation {
	Union,
	Difference,
	Intersection,
	SubtractFront,
	SubtractBack,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum BooleanOperationError {
	InvalidSelection,
	InvalidIntersections,
	NoIntersections,
	NothingDone, // not neccesarily an error
	DirectionUndefined,
	Unexpected, // for debugging, when complete nothing should be unexpected
}

/// A simple and idiomatic way to write short "if Let" statements
macro_rules! do_if {
	($option:expr, $name:ident{$todo:expr}) => {
		if let Some($name) = $option {
			$todo
		}
	};
}

struct Edge {
	pub from: Origin,
	pub destination: usize,
	pub curve: BezPath,
}

impl Debug for Edge {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		f.write_str(format!("\n    To: {}, Type: {:?}", self.destination, self.from).as_str())?;
		f.write_str(format!("    {:?}", self.curve).as_str())
	}
}

struct Vertex {
	pub intersect: Intersect,
	pub edges: Vec<Edge>,
}

impl Debug for Vertex {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.write_str(format!("\n    Intersect@ {:?}", self.intersect.point).as_str())?;
		f.debug_list().entries(self.edges.iter()).finish()
	}
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum Direction {
	CCW,
	CW,
}

/// Behavior: Intersection and Union cases are distinuguished between by cycle area magnitude
///   - This only effects shapes whose intersection is a single shape, and the intersection is similalarly sized to the union
///   - can be solved by first computing at low accuracy, and if the values are close recomputing.
#[derive(Clone)]
struct Cycle {
	vertices: Vec<(usize, Origin)>,
	dir: Option<Direction>,
	area: f64,
}

impl Cycle {
	pub fn new(vidx: usize, eorg: Origin) -> Self {
		Cycle {
			vertices: vec![(vidx, eorg)],
			dir: None,
			area: 0.0,
		}
	}

	/// returns true when the cycle is complete, a cycle is complete when it revisits its first vertex
	/// where edge is the edge traversed in order to get to vertex
	/// for purposes of computing direction this function assumes vertices are traversed in order
	fn extend(&mut self, vertex: usize, edge_origin: Origin, edge_curve: &BezPath) -> bool {
		self.vertices.push((vertex, edge_origin));
		self.area += path_area(edge_curve);
		vertex == self.vertices[0].0
	}

	/// returns number of vertices == number of edges in cycle
	fn len(&self) -> usize {
		self.vertices.len() - 1
	}

	pub fn prev_edge_origin(&self) -> Origin {
		self.vertices.last().unwrap().1
	}

	pub fn prev_vertex(&self) -> usize {
		self.vertices.last().unwrap().0
	}

	pub fn vertices(&self) -> &Vec<(usize, Origin)> {
		&self.vertices
	}

	pub fn area(&self) -> f64 {
		self.area
	}

	pub fn direction(&mut self) -> Result<Direction, BooleanOperationError> {
		match self.dir {
			Some(direction) => Ok(direction),
			None => {
				if self.area > 0.0 {
					self.dir = Some(Direction::CCW);
					Ok(Direction::CCW)
				} else if self.area < 0.0 {
					self.dir = Some(Direction::CW);
					Ok(Direction::CW)
				} else {
					Err(BooleanOperationError::DirectionUndefined)
				}
			}
		}
	}

	/// - if the path is empty (has no segments) the function Errs
	/// - if the path crosses itself the computed direction may be (probably will be) wrong, on account of it not really being defined
	pub fn direction_for_path(path: &BezPath) -> Result<Direction, BooleanOperationError> {
		let mut area = 0.0;
		path.segments().for_each(|seg| area += seg.signed_area());
		if area > 0.0 {
			Ok(Direction::CCW)
		} else if area < 0.0 {
			Ok(Direction::CW)
		} else {
			Err(BooleanOperationError::DirectionUndefined)
		}
	}
}

/// Optimization: store computed segment bounding boxes, or even edge bounding boxes to prevent recomputation
#[derive(Debug)]
struct PathGraph {
	vertices: Vec<Vertex>,
}

/// Boolean Operation Algorithm
///   - Behavior: Has somewhat (totally?) undefined behavior when shapes have self intersections
/// PathGraph: represents a directional graph with edges "colored" by Origin
/// each edge also represents a portion of a visible shape
/// ! remove this allow @ release
#[allow(dead_code)]
impl PathGraph {
	pub fn from_paths(alpha: &BezPath, beta: &BezPath) -> Result<PathGraph, BooleanOperationError> {
		//TODO: check for closed paths somewhere, maybe here?
		let mut new = PathGraph {
			vertices: intersections(alpha, beta).into_iter().map(|i| Vertex { intersect: i, edges: Vec::new() }).collect(),
		};
		// we only consider graphs with even numbers of intersections.
		// An odd number of intersections occurrs when either
		//    1. There exists a tangential intersection (which shouldn't effect boolean operations)
		//    2. The algorithm has found an extra intersection or missed an intersection
		if new.size() == 0 {
			return Err(BooleanOperationError::NoIntersections);
		}
		if new.size() % 2 != 0 {
			return Err(BooleanOperationError::InvalidIntersections);
		}
		new.add_edges_from_path(alpha, Origin::Alpha);
		new.add_edges_from_path(beta, Origin::Beta);
		// log::debug!("size: {}, {:?}", new.size(), new);
		Ok(new)
	}

	/// NOTE: about intersection time_val order
	/// !Panics when path is empty
	/// !closepaths aren't being properly removed maye?
	fn add_edges_from_path(&mut self, path: &BezPath, origin: Origin) {
		struct AlgorithmState {
			//cstart holds the idx of the vertex the current edge is starting from
			cstart: Option<usize>,
			current: Vec<PathSeg>,
			// in order to iterate through once, store information for incomplete first edge
			beginning: Vec<PathSeg>,
			start_idx: Option<usize>,
			// seg idx != el_idx
			seg_idx: i32,
		}

		impl AlgorithmState {
			fn new() -> Self {
				AlgorithmState {
					cstart: None,
					current: Vec::new(),
					beginning: Vec::new(),
					start_idx: None,
					seg_idx: 0,
				}
			}

			fn reset(&mut self) {
				self.cstart = None;
				self.current = Vec::new();
				self.beginning = Vec::new();
				self.start_idx = None;
			}

			fn advance_by_seg(&mut self, graph: &mut PathGraph, seg: PathSeg, origin: Origin) {
				let (v_ids, mut t_values) = graph.intersects_in_seg(self.seg_idx, origin);
				if !v_ids.is_empty() {
					let sub_segs = subdivide_path_seg(&seg, &mut t_values);
					for (vertex_id, sub_seg) in v_ids.into_iter().zip(sub_segs.iter()) {
						match self.cstart {
							Some(idx) => {
								do_if!(sub_seg, end_of_edge { self.current.push(*end_of_edge)});
								graph.add_edge(origin, idx, vertex_id, self.current.clone());
								self.cstart = Some(vertex_id);
								self.current = Vec::new();
							}
							None => {
								self.cstart = Some(vertex_id);
								self.start_idx = Some(vertex_id);
								do_if!(sub_seg, end_of_begining {self.beginning.push(*end_of_begining)});
							}
						}
					}
					do_if!(sub_segs.last().unwrap(), start_of_edge {self.current.push(*start_of_edge)});
				} else {
					match self.cstart {
						Some(_) => self.current.push(seg),
						None => self.beginning.push(seg),
					}
				}
				self.seg_idx += 1;
			}

			fn advance_by_closepath(&mut self, graph: &mut PathGraph, initial_point: &mut Point, origin: Origin) {
				// *when a curve ends in a closepath and its start point does not equal its enpoint they should be connected with a line
				let end_seg = match self.current.last() {
					Some(seg) => seg,
					None => self.beginning.last().unwrap(), // if both current and beginning are empty, the path is empty
				};
				let temp_copy = end_seg.end();
				if temp_copy != *initial_point {
					// a closepath implicitly defines a line which closes the path
					self.advance_by_seg(graph, PathSeg::Line(Line { p0: temp_copy, p1: *initial_point }), origin);
				}
				// when a closepath is not followed by moveto, the next startpath starts at the end of the current path
				*initial_point = temp_copy;
			}

			fn finalize_sub_path(&mut self, graph: &mut PathGraph, origin: Origin) {
				if let (Some(cstart_), Some(start_idx_)) = (self.cstart, self.start_idx) {
					//complete the current path
					self.current.append(&mut self.beginning);
					graph.add_edge(origin, cstart_, start_idx_, self.current.clone());
				} else {
					//path has a subpath with no intersects
					//create a dummy vertex with single edge which will be identified as cycle
					let dumb_id = graph.add_vertex(Intersect::new(self.beginning[0].start(), 0.0, 0.0, -1, -1));
					graph.add_edge(origin, dumb_id, dumb_id, self.beginning.clone());
				}
			}
		}

		let mut algorithm_state = AlgorithmState::new();

		let mut initial_point = Point::new(0.0, 0.0);

		for (el_idx, el) in path.iter().enumerate() {
			match el {
				PathEl::MoveTo(p) => initial_point = p,
				PathEl::ClosePath => {
					algorithm_state.advance_by_closepath(self, &mut initial_point, origin);

					algorithm_state.finalize_sub_path(self, origin);

					algorithm_state.reset();
				}
				_ => {
					algorithm_state.advance_by_seg(self, path.get_seg(el_idx).unwrap(), origin);
				}
			}
		}
	}

	fn add_vertex(&mut self, intersect: Intersect) -> usize {
		self.vertices.push(Vertex { intersect, edges: Vec::new() });
		self.vertices.len() - 1
	}

	fn add_edge(&mut self, origin: Origin, vertex: usize, destination: usize, curve: Vec<PathSeg>) {
		let new_edge = Edge {
			from: origin,
			destination,
			curve: BezPath::from_path_segments(curve.into_iter()),
		};
		self.vertices[vertex].edges.push(new_edge);
	}

	/// returns the Vertex idx and intersect t-value for all intersects in segment identified by seg_idx from origin
	/// sorts both lists for ascending t_value
	fn intersects_in_seg(&self, seg_idx: i32, origin: Origin) -> (Vec<usize>, Vec<f64>) {
		let mut vertice_idx = Vec::new();
		let mut t_values = Vec::new();
		for (v_idx, vertex) in self.vertices.iter().enumerate() {
			if vertex.intersect.seg_idx(origin) == seg_idx {
				let next_t = vertex.intersect.t_val(origin);
				let insert_idx = match t_values.binary_search_by(|val: &f64| (*val).partial_cmp(&next_t).unwrap_or(std::cmp::Ordering::Less)) {
					Ok(val) | Err(val) => val,
				};
				t_values.insert(insert_idx, next_t);
				vertice_idx.insert(insert_idx, v_idx)
			}
		}
		(vertice_idx, t_values)
	}

	// return number of vertices in graph, this is equivalent to the number of intersections
	pub fn size(&self) -> usize {
		return self.vertices.len();
	}

	pub fn vertex(&self, idx: usize) -> &Vertex {
		&self.vertices[idx]
	}

	/// a properly constructed PathGraph has no duplicate edges of the same Origin
	pub fn edge(&self, from: usize, to: usize, origin: Origin) -> Option<&Edge> {
		// with a data strucutre restructure, or a hashmap, the find here could be avoided
		// but it probably has a miniaml performance impact
		self.vertex(from).edges.iter().find(|edge| edge.destination == to && edge.from == origin)
	}

	/// return reference to intersect associated with the vertex at idx
	pub fn intersect(&self, idx: usize) -> &Intersect {
		&self.vertices[idx].intersect
	}

	/// where a valid cycle alternates edge Origin
	/// Single edge/Singel vertex 'dummy' cycles are also valid
	fn get_cycle(&self, cycle: &mut Cycle, marker_map: &mut Vec<u8>) {
		if cycle.prev_edge_origin() == Origin::Alpha {
			marker_map[cycle.prev_vertex()] |= 1;
		} else {
			marker_map[cycle.prev_vertex()] |= 2;
		}
		if let Some(next_edge) = self.vertex(cycle.prev_vertex()).edges.iter().find(|edge| edge.from != cycle.prev_edge_origin()) {
			if !cycle.extend(next_edge.destination, next_edge.from, &next_edge.curve) {
				self.get_cycle(cycle, marker_map)
			}
		}
	}

	pub fn get_cycles(&self) -> Vec<Cycle> {
		let mut cycles = Vec::new();
		let mut markers = Vec::new();
		markers.resize(self.size(), 0);

		self.vertices.iter().enumerate().for_each(|(vertex_idx, _vertex)| {
			if (markers[vertex_idx] & 1) == 0 {
				let mut temp = Cycle::new(vertex_idx, Origin::Alpha);
				self.get_cycle(&mut temp, &mut markers);
				if temp.len() > 0 {
					cycles.push(temp);
				}
			}
			if (markers[vertex_idx] & 2) == 0 {
				let mut temp = Cycle::new(vertex_idx, Origin::Beta);
				self.get_cycle(&mut temp, &mut markers);
				if temp.len() > 0 {
					cycles.push(temp);
				}
			}
		});
		cycles
	}

	pub fn get_shape(&self, cycle: &Cycle, style: &PathStyle) -> Shape {
		let mut curve = Vec::new();
		let vertices = cycle.vertices();
		for idx in 1..vertices.len() {
			// we expect the cycle to be valid, this should not panic
			concat_paths(&mut curve, &self.edge(vertices[idx - 1].0, vertices[idx].0, vertices[idx].1).unwrap().curve);
		}
		curve.push(PathEl::ClosePath);
		Shape::from_bez_path(BezPath::from_vec(curve), *style, false)
	}
}

/// if t is on (0, 1), returns the splitcurvepath
/// if t is outside [0, 1], returns None, None
/// otherwise returns the whole path
/// TODO: test values outside 1
pub fn split_path_seg(p: &PathSeg, t: f64) -> (Option<PathSeg>, Option<PathSeg>) {
	if t <= F64PRECISION {
		if t >= 1.0 - F64PRECISION {
			return (None, None);
		}
		return (Some(*p), None);
	} else if t >= 1.0 - F64PRECISION {
		return (None, Some(*p));
	}
	match p {
		PathSeg::Cubic(cubic) => {
			let a1 = Line::new(cubic.p0, cubic.p1).eval(t);
			let a2 = Line::new(cubic.p1, cubic.p2).eval(t);
			let a3 = Line::new(cubic.p2, cubic.p3).eval(t);
			let b1 = Line::new(a1, a2).eval(t);
			let b2 = Line::new(a2, a3).eval(t);
			let c1 = Line::new(b1, b2).eval(t);
			(
				Some(PathSeg::Cubic(CubicBez { p0: cubic.p0, p1: a1, p2: b1, p3: c1 })),
				Some(PathSeg::Cubic(CubicBez { p0: c1, p1: b2, p2: a3, p3: cubic.p3 })),
			)
		}
		PathSeg::Quad(quad) => {
			let b1 = Line::new(quad.p0, quad.p1).eval(t);
			let b2 = Line::new(quad.p1, quad.p2).eval(t);
			let c1 = Line::new(b1, b2).eval(t);
			(
				Some(PathSeg::Quad(QuadBez { p0: quad.p0, p1: b1, p2: c1 })),
				Some(PathSeg::Quad(QuadBez { p0: c1, p1: b2, p2: quad.p2 })),
			)
		}
		PathSeg::Line(line) => {
			let split = line.eval(t);
			(Some(PathSeg::Line(Line { p0: line.p0, p1: split })), Some(PathSeg::Line(Line { p0: split, p1: line.p1 })))
		}
	}
}

/// splits p at each of t_vals
/// t_vals should be sorted in ascending order
/// the length of the returned vector is equal to 1 + t_vals.len()
pub fn subdivide_path_seg(p: &PathSeg, t_vals: &mut [f64]) -> Vec<Option<PathSeg>> {
	let mut sub_segs = Vec::new();
	let mut to_split = Some(*p);
	let mut prev_split = 0.0;
	for split in t_vals {
		if let Some(unhewn) = to_split {
			let (sub_seg, _to_split) = split_path_seg(&unhewn, (*split - prev_split) / (1.0 - prev_split));
			to_split = _to_split;
			sub_segs.push(sub_seg);
			prev_split = *split;
		} else {
			sub_segs.push(None);
		}
	}
	sub_segs.push(to_split);
	sub_segs
}

/// !check if shapes are filled
/// !Bug: shape with at least two subpaths and comprised of many unions sometimes has erroneous movetos emmbedded in edges
pub fn boolean_operation(select: BooleanOperation, mut alpha: Shape, mut beta: Shape) -> Result<Vec<Shape>, BooleanOperationError> {
	if alpha.path.is_empty() || beta.path.is_empty() {
		return Err(BooleanOperationError::InvalidSelection);
	}
	alpha.path = close_path(&alpha.path);
	beta.path = close_path(&beta.path);
	let alpha_dir = Cycle::direction_for_path(&alpha.path)?;
	let beta_dir = Cycle::direction_for_path(&beta.path)?;
	match select {
		BooleanOperation::Union => {
			match if beta_dir == alpha_dir {
				PathGraph::from_paths(&alpha.path, &beta.path)
			} else {
				PathGraph::from_paths(&alpha.path, &reverse_path(&beta.path))
			} {
				Ok(graph) => {
					let mut cycles = graph.get_cycles();
					// "extra calls to ParamCurveArea::area here"
					let mut boolean_union = graph.get_shape(
						cycles.iter().reduce(|max, cycle| if cycle.area().abs() >= max.area().abs() { cycle } else { max }).unwrap(),
						&alpha.style,
					);
					for interior in collect_shapes(&graph, &mut cycles, |dir| dir != alpha_dir, |_| &alpha.style)? {
						add_subpath(&mut boolean_union.path, interior.path);
					}
					Ok(vec![boolean_union])
				}
				Err(BooleanOperationError::NoIntersections) => {
					//if shape is inside the other the Union is just the larger
					//check could also be done with area and single ray cast
					if cast_horizontal_ray(point_on_curve(&beta.path), &alpha.path) % 2 != 0 {
						Ok(vec![alpha])
					} else if cast_horizontal_ray(point_on_curve(&alpha.path), &beta.path) % 2 != 0 {
						beta.style = alpha.style;
						Ok(vec![beta])
					} else {
						Err(BooleanOperationError::NothingDone)
					}
				}
				Err(err) => Err(err),
			}
		}
		BooleanOperation::Difference => {
			let graph = if beta_dir != alpha_dir {
				PathGraph::from_paths(&alpha.path, &beta.path)?
			} else {
				PathGraph::from_paths(&alpha.path, &reverse_path(&beta.path))?
			};
			collect_shapes(&graph, &mut graph.get_cycles(), |_| true, |dir| if dir == alpha_dir { &alpha.style } else { &beta.style })
		}
		BooleanOperation::Intersection => {
			match if beta_dir == alpha_dir {
				PathGraph::from_paths(&alpha.path, &beta.path)
			} else {
				PathGraph::from_paths(&alpha.path, &reverse_path(&beta.path))
			} {
				Ok(graph) => {
					let mut cycles = graph.get_cycles();
					// "extra calls to ParamCurveArea::area here"
					cycles.remove(
						cycles
							.iter()
							.enumerate()
							.reduce(|(midx, max), (idx, cycle)| if cycle.area().abs() >= max.area().abs() { (idx, cycle) } else { (midx, max) })
							.unwrap()
							.0,
					);
					collect_shapes(&graph, &mut cycles, |dir| dir == alpha_dir, |_| &alpha.style)
				}
				Err(BooleanOperationError::NoIntersections) => {
					//check could also be done with area and single ray cast
					if cast_horizontal_ray(point_on_curve(&beta.path), &alpha.path) % 2 != 0 {
						beta.style = alpha.style;
						Ok(vec![beta])
					} else if cast_horizontal_ray(point_on_curve(&alpha.path), &beta.path) % 2 != 0 {
						Ok(vec![alpha])
					} else {
						Err(BooleanOperationError::NothingDone)
					}
				}
				Err(err) => Err(err),
			}
		}
		BooleanOperation::SubtractBack => {
			match if beta_dir != alpha_dir {
				PathGraph::from_paths(&alpha.path, &beta.path)
			} else {
				PathGraph::from_paths(&alpha.path, &reverse_path(&beta.path))
			} {
				Ok(graph) => collect_shapes(&graph, &mut graph.get_cycles(), |dir| dir != alpha_dir, |_| &beta.style),
				Err(BooleanOperationError::NoIntersections) => {
					if cast_horizontal_ray(point_on_curve(&alpha.path), &beta.path) % 2 != 0 {
						add_subpath(&mut beta.path, if beta_dir == alpha_dir { reverse_path(&alpha.path) } else { alpha.path });
						beta.style = alpha.style;
						Ok(vec![beta])
					} else {
						Err(BooleanOperationError::NothingDone)
					}
				}
				Err(err) => Err(err),
			}
		}
		BooleanOperation::SubtractFront => {
			match if beta_dir != alpha_dir {
				PathGraph::from_paths(&alpha.path, &beta.path)
			} else {
				PathGraph::from_paths(&alpha.path, &reverse_path(&beta.path))
			} {
				Ok(graph) => collect_shapes(&graph, &mut graph.get_cycles(), |dir| dir == alpha_dir, |_| &alpha.style),
				Err(BooleanOperationError::NoIntersections) => {
					if cast_horizontal_ray(point_on_curve(&beta.path), &alpha.path) % 2 != 0 {
						add_subpath(&mut alpha.path, if beta_dir == alpha_dir { reverse_path(&beta.path) } else { beta.path });
						Ok(vec![alpha])
					} else {
						Err(BooleanOperationError::NothingDone)
					}
				}
				Err(err) => Err(err),
			}
		}
	}
}

/// !May be a bug where ray is cast into the endpoints of two lines
pub fn cast_horizontal_ray(from: Point, into: &BezPath) -> usize {
	let mut num = 0;
	let ray = Line {
		p0: from,
		p1: Point { x: from.x + 1.0, y: from.y },
	};
	let mut intersects = Vec::new();
	for ref seg in into.segments() {
		// * could check bounding boxes more rigorously
		if seg.bounding_box().x1 > from.x {
			line_curve_intersections(&ray, seg, true, |_, b| valid_t(b), &mut intersects);
		}
	}
	for intersect in intersects {
		if valid_t(intersect.t_val(Origin::Beta)) {
			num += 1;
		}
	}
	num
}

/// * uses curve start point as point on the curve
/// ! Panics if the curve is empty
pub fn point_on_curve(curve: &BezPath) -> Point {
	curve.segments().nth(0).unwrap().start()
}

/// panics if the curve has no PathSeg's
pub fn bounding_box(curve: &BezPath) -> Rect {
	curve
		.segments()
		.map(|seg| <PathSeg as ParamCurveExtrema>::bounding_box(&seg))
		.reduce(|bounds, rect| bounds.union(rect))
		.unwrap()
}

fn collect_shapes<'a, F, G>(graph: &PathGraph, cycles: &mut Vec<Cycle>, predicate: F, style: G) -> Result<Vec<Shape>, BooleanOperationError>
where
	F: Fn(Direction) -> bool,
	G: Fn(Direction) -> &'a PathStyle,
{
	let mut shapes = Vec::new();
	if cycles.len() == 0 {
		return Err(BooleanOperationError::Unexpected);
	}
	for cycle in cycles {
		match cycle.direction() {
			Ok(dir) => {
				if predicate(dir) {
					shapes.push(graph.get_shape(cycle, style(dir)));
				}
			}
			Err(err) => return Err(err),
		}
	}
	Ok(shapes)
}

pub fn reverse_pathseg(seg: &mut PathSeg) {
	match seg {
		PathSeg::Line(line) => std::mem::swap(&mut line.p0, &mut line.p1),
		PathSeg::Quad(quad) => std::mem::swap(&mut quad.p0, &mut quad.p1),
		PathSeg::Cubic(cubic) => {
			std::mem::swap(&mut cubic.p0, &mut cubic.p3);
			std::mem::swap(&mut cubic.p1, &mut cubic.p2);
		}
	}
}

/// reverse path by reversing each PathSeg, and reversing the order of PathSegs within each subpath
pub fn reverse_path(path: &BezPath) -> BezPath {
	let mut curve = Vec::new();
	let mut temp = Vec::new();
	let mut segs = path.segments();

	for element in path.iter() {
		match element {
			PathEl::MoveTo(_) => {
				curve.append(&mut temp);
				temp = Vec::new();
			}
			_ => {
				if let Some(mut seg) = segs.next() {
					reverse_pathseg(&mut seg);
					temp.push(seg);
				}
			}
		}
	}
	curve.append(&mut temp);
	BezPath::from_path_segments(curve.into_iter().rev())
}

pub fn is_closed(curve: &BezPath) -> bool {
	curve.iter().last() == Some(PathEl::ClosePath)
}

/// Close off all subpaths in curve by inserting a ClosePath whenever a MoveTo is not preceded by one
pub fn close_path(curve: &BezPath) -> BezPath {
	let mut new = BezPath::new();
	let mut path_closed_flag = true;
	for el in curve.iter().skip(1) {
		match el {
			PathEl::MoveTo(p) => {
				if !path_closed_flag {
					new.push(PathEl::ClosePath);
				}
				new.push(PathEl::MoveTo(p));
				path_closed_flag = false;
			}
			PathEl::ClosePath => {
				path_closed_flag = true;
				new.push(PathEl::ClosePath);
			}
			element => {
				new.push(element);
			}
		}
	}
	if !path_closed_flag {
		new.push(PathEl::ClosePath);
	}
	new
}

/// concat b to a, where b is not a new subpath but a continuation of a
pub fn concat_paths(a: &mut Vec<PathEl>, b: &BezPath) {
	if a.is_empty() {
		a.append(&mut b.elements().to_vec());
		return;
	}
	// remove closepath
	if let Some(PathEl::ClosePath) = a.last() {
		a.remove(a.len() - 1);
	}
	// skip inital moveto, which should be guarenteed to exist
	b.iter().skip(1).for_each(|element| a.push(element));
}

/// concat b to a, where b is a new subpath
pub fn add_subpath(a: &mut BezPath, b: BezPath) {
	b.into_iter().for_each(|el| a.push(el));
}

pub fn path_length(a: &BezPath, accuracy: Option<f64>) -> f64 {
	let mut sum = 0.0;
	//computing arclen with F64PRECISION accuracy is probably ridiculous
	match accuracy {
		Some(val) => a.segments().for_each(|seg| sum += seg.arclen(val)),
		None => a.segments().for_each(|seg| sum += seg.arclen(F64PRECISION)),
	}
	sum
}

pub fn path_area(a: &BezPath) -> f64 {
	a.segments().fold(0.0, |mut area, seg| {
		area += seg.signed_area();
		area
	})
}
