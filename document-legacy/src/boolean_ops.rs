use crate::consts::F64PRECISE;
use crate::intersection::{intersections, line_curve_intersections, valid_t, Intersect, Origin};
use crate::layers::shape_layer::ShapeLayer;
use crate::layers::style::PathStyle;

use kurbo::{BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveArea, ParamCurveExtrema, PathEl, PathSeg, Point, QuadBez, Rect};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fmt::{self, Debug, Formatter};
use std::mem::swap;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum BooleanOperation {
	Union,
	Difference,
	Intersection,
	SubtractFront,
	SubtractBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum BooleanOperationError {
	InvalidSelection,
	InvalidIntersections,
	NoIntersections,
	NothingDone, // Not necessarily an error
	DirectionUndefined,
	NoResult,
	Unexpected, // For debugging, when complete nothing should be unexpected
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
		f.write_str(
			format!(
				"\n    Intersect Point: {:?} Segment index of A: {:?}, Segment index of B: {:?} t value of A: {:?} t value of B: {:?}",
				self.intersect.point,
				self.intersect.segment_index(Origin::Alpha),
				self.intersect.segment_index(Origin::Beta),
				self.intersect.t_value(Origin::Alpha),
				self.intersect.t_value(Origin::Beta),
			)
			.as_str(),
		)?;
		f.debug_list().entries(self.edges.iter()).finish()
	}
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum Direction {
	Ccw,
	Cw,
}

/// Behavior: Intersection and Union cases are distinguished between by cycle area magnitude.
/// This only affects shapes whose intersection is a single shape, and the intersection is similarly sized to the union.
/// Can be solved by first computing at low accuracy, and if the values are close recomputing.
#[derive(Clone)]
struct Cycle {
	vertices: Vec<(usize, Origin)>,
	direction: Option<Direction>,
	area: f64,
}

impl Cycle {
	pub fn new(start_vertex_index: usize, edge_origin: Origin) -> Self {
		Cycle {
			vertices: vec![(start_vertex_index, edge_origin)],
			direction: None,
			area: 0.0,
		}
	}

	/// Returns true when the cycle is complete, a cycle is complete when it revisits its first vertex where edge is the edge traversed in order to get to vertex.
	/// For purposes of computing direction this function assumes vertices are traversed in order
	fn extend(&mut self, vertex: usize, edge_origin: Origin, edge_curve: &BezPath) -> bool {
		self.vertices.push((vertex, edge_origin));
		self.area += path_area(edge_curve);
		vertex == self.vertices[0].0
	}

	/// Returns number of vertices == number of edges in cycle.
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
		match self.direction {
			Some(direction) => Ok(direction),
			None => {
				if self.area > 0.0 {
					self.direction = Some(Direction::Ccw);
					Ok(Direction::Ccw)
				} else if self.area < 0.0 {
					self.direction = Some(Direction::Cw);
					Ok(Direction::Cw)
				} else {
					Err(BooleanOperationError::DirectionUndefined)
				}
			}
		}
	}

	/// If the path is empty (has no segments), the function `Err`s.
	/// If the path crosses itself, the computed direction may (or probably will) be wrong, on account of it not really being defined.
	pub fn direction_for_path(path: &BezPath) -> Result<Direction, BooleanOperationError> {
		let mut area = 0.0;
		path.segments().for_each(|path_segment| area += path_segment.signed_area());
		if area > 0.0 {
			Ok(Direction::Ccw)
		} else if area < 0.0 {
			Ok(Direction::Cw)
		} else {
			Err(BooleanOperationError::DirectionUndefined)
		}
	}
}

/// Optimization: store computed segment bounding boxes, or even edge bounding boxes to prevent recomputation.
#[derive(Debug)]
struct PathGraph {
	vertices: Vec<Vertex>,
}

/// # Boolean Operation Algorithm
/// `PathGraph` represents a directional graph with edges "colored" by `Origin`.
/// Each edge also represents a portion of a visible shape.
/// Has somewhat (totally?) undefined behavior when shapes have self intersections.
impl PathGraph {
	pub fn from_paths(alpha: &BezPath, beta: &BezPath) -> Result<PathGraph, BooleanOperationError> {
		let mut new = PathGraph {
			vertices: intersections(alpha, beta).into_iter().map(|i| Vertex { intersect: i, edges: Vec::new() }).collect(),
		};
		// We only consider graphs with even numbers of intersections.
		// An odd number of intersections occurs when either:
		// 1. There exists a tangential intersection (which shouldn't affect boolean ops)
		// 2. The algorithm has found an extra intersection or missed an intersection
		if new.size() == 0 {
			return Err(BooleanOperationError::NoIntersections);
		}
		if new.size() % 2 != 0 {
			return Err(BooleanOperationError::InvalidIntersections);
		}
		new.add_edges_from_path(alpha, Origin::Alpha);
		new.add_edges_from_path(beta, Origin::Beta);
		Ok(new)
	}

	// TODO: NOTE: about intersection time_val order
	/// Expects `path` (and all subpaths in `path`) to be closed.
	/// # Panics
	/// This function panics when `path` is empty.
	fn add_edges_from_path(&mut self, path: &BezPath, origin: Origin) {
		struct AlgorithmState {
			//current_start holds the index of the vertex the current edge is starting from
			current_start: Option<usize>,
			current: Vec<PathSeg>,
			// in order to iterate through once, store information for incomplete first edge
			beginning: Vec<PathSeg>,
			start_index: Option<usize>,
			// seg index != el_index
			seg_index: i32,
		}

		impl AlgorithmState {
			fn new() -> Self {
				AlgorithmState {
					current_start: None,
					current: Vec::new(),
					beginning: Vec::new(),
					start_index: None,
					seg_index: 0,
				}
			}

			fn reset(&mut self) {
				self.current_start = None;
				self.current = Vec::new();
				self.beginning = Vec::new();
				self.start_index = None;
			}

			fn advance_by_seg(&mut self, graph: &mut PathGraph, seg: PathSeg, origin: Origin) {
				let (vertex_ids, mut t_values) = graph.intersects_in_seg(self.seg_index, origin);
				if !vertex_ids.is_empty() {
					let subdivided = subdivide_path_seg(&seg, &mut t_values);
					for (vertex_id, sub_seg) in vertex_ids.into_iter().zip(subdivided.iter()) {
						match self.current_start {
							Some(index) => {
								sub_seg.map(|end_of_edge| self.current.push(end_of_edge));
								graph.add_edge(origin, index, vertex_id, self.current.clone());
								self.current_start = Some(vertex_id);
								self.current = Vec::new();
							}
							None => {
								self.current_start = Some(vertex_id);
								self.start_index = Some(vertex_id);
								sub_seg.map(|end_of_beginning| self.beginning.push(end_of_beginning));
							}
						}
					}
					subdivided.last().unwrap().map(|start_of_edge| self.current.push(start_of_edge));
				} else {
					match self.current_start {
						Some(_) => self.current.push(seg),
						None => self.beginning.push(seg),
					}
				}
				self.seg_index += 1;
			}

			fn advance_by_closepath(&mut self, graph: &mut PathGraph, initial_point: &mut Point, origin: Origin) {
				// When a curve ends in a closepath and its start point does not equal its endpoint they should be connected with a line
				let last_line = match self.current.last() {
					Some(start_of_final_edge) => Line {
						p0: start_of_final_edge.end(),
						p1: *initial_point,
					},
					None => {
						// When None occurs the current edge has been connected to a vertex.
						// Either self.beginning is Some or None, if self.beginning is Some there may be a dangling edge to connect
						// if self.beginning is None, the end of the current edge may not have closed the path
						match self.beginning.last() {
							Some(end_of_first_edge) => Line {
								p0: end_of_first_edge.end(),
								p1: *initial_point,
							},
							None => Line {
								// should never panic, either a intersection has been encountered, so self.current_start is Some.
								// or no vertex has been encountered so self.beginning.last() is Some
								p0: graph.vertex(self.current_start.unwrap()).intersect.point,
								p1: *initial_point,
							},
						}
					}
				};
				if last_line.length() > F64PRECISE {
					// A closepath implicitly defines a line which closes the path and the closepath line may contain intersections
					self.advance_by_seg(graph, PathSeg::Line(last_line), origin);
				}
			}

			fn finalize_sub_path(&mut self, graph: &mut PathGraph, origin: Origin) {
				if let (Some(current_start_), Some(start_index_)) = (self.current_start, self.start_index) {
					// Complete the current path
					self.current.append(&mut self.beginning);
					graph.add_edge(origin, current_start_, start_index_, self.current.clone());
				} else {
					// Path has a subpath with no intersects.
					// Create a dummy vertex with single edge which will be identified as cycle.
					let dumb_id = graph.add_vertex(Intersect::new(self.beginning[0].start(), 0.0, 0.0, -1, -1));
					graph.add_edge(origin, dumb_id, dumb_id, self.beginning.clone());
				}
			}
		}

		let mut algorithm_state = AlgorithmState::new();

		// All valid SVG paths start with a moveto, so this will always be initialized
		let mut initial_point = Point::new(0.0, 0.0);

		for (el_index, el) in path.iter().enumerate() {
			match el {
				PathEl::MoveTo(p) => initial_point = p,
				PathEl::ClosePath => {
					algorithm_state.advance_by_closepath(self, &mut initial_point, origin);

					algorithm_state.finalize_sub_path(self, origin);

					algorithm_state.reset();
				}
				_ => {
					algorithm_state.advance_by_seg(self, path.get_seg(el_index).unwrap(), origin);
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

	/// Returns the `Vertex` index and intersect `t_value` for all intersects in the segment identified by `seg_index` from `origin`.
	/// Sorts both lists for ascending `t_value`.
	fn intersects_in_seg(&self, seg_index: i32, origin: Origin) -> (Vec<usize>, Vec<f64>) {
		let mut vertex_index = Vec::new();
		let mut t_values = Vec::new();
		for (v_index, vertex) in self.vertices.iter().enumerate() {
			if vertex.intersect.segment_index(origin) == seg_index {
				let next_t = vertex.intersect.t_value(origin);
				let insert_index = match t_values.binary_search_by(|val: &f64| (*val).partial_cmp(&next_t).unwrap_or(std::cmp::Ordering::Less)) {
					Ok(val) | Err(val) => val,
				};
				t_values.insert(insert_index, next_t);
				vertex_index.insert(insert_index, v_index)
			}
		}
		(vertex_index, t_values)
	}

	/// Returns the number of vertices in the graph. This is equivalent to the number of intersections.
	pub fn size(&self) -> usize {
		self.vertices.len()
	}

	pub fn vertex(&self, index: usize) -> &Vertex {
		&self.vertices[index]
	}

	/// A properly constructed `PathGraph` has no duplicate edges of the same `Origin`.
	pub fn edge(&self, from: usize, to: usize, origin: Origin) -> Option<&Edge> {
		// With a data structure restructure, or a hashmap, the `find()` here could be avoided, but it probably has a minimal performance impact
		self.vertex(from).edges.iter().find(|edge| edge.destination == to && edge.from == origin)
	}

	/// Where a valid cycle alternates edge `Origin`.
	/// Single edge/single vertex "dummy" cycles are also valid.
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

		self.vertices.iter().enumerate().for_each(|(vertex_index, _vertex)| {
			if (markers[vertex_index] & 1) == 0 {
				let mut temp = Cycle::new(vertex_index, Origin::Alpha);
				self.get_cycle(&mut temp, &mut markers);
				if temp.len() > 0 {
					cycles.push(temp);
				}
			}
			if (markers[vertex_index] & 2) == 0 {
				let mut temp = Cycle::new(vertex_index, Origin::Beta);
				self.get_cycle(&mut temp, &mut markers);
				if temp.len() > 0 {
					cycles.push(temp);
				}
			}
		});
		cycles
	}

	pub fn get_shape(&self, cycle: &Cycle, style: &PathStyle) -> ShapeLayer {
		let mut curve = Vec::new();
		let vertices = cycle.vertices();
		for index in 1..vertices.len() {
			// We expect the cycle to be valid so this should not panic
			concat_paths(&mut curve, &self.edge(vertices[index - 1].0, vertices[index].0, vertices[index].1).unwrap().curve);
		}
		curve.push(PathEl::ClosePath);
		ShapeLayer::new(BezPath::from_vec(curve).iter().into(), style.clone())
	}
}

/// If `t` is on `(0, 1)`, returns the split curve.
/// If `t` is outside `[0, 1]`, returns `(None, None)`
/// If `t` is 0 returns `(None, p)`.
/// If `t` is 1 returns `(p, None)`.
pub fn split_path_seg(p: &PathSeg, t: f64) -> (Option<PathSeg>, Option<PathSeg>) {
	if t <= -F64PRECISE || t >= 1.0 + F64PRECISE {
		return (None, None);
	}
	if t <= F64PRECISE {
		return (None, Some(*p));
	}
	if t >= 1.0 - F64PRECISE {
		return (Some(*p), None);
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

/// Splits `p` at each of `t_values`.
/// `t_values` should be sorted in ascending order.
/// The length of the returned `Vec` is always equal to `1 + t_values.len()`.
pub fn subdivide_path_seg(p: &PathSeg, t_values: &mut [f64]) -> Vec<Option<PathSeg>> {
	let mut sub_segments = Vec::new();
	let mut to_split = Some(*p);
	let mut prev_split = 0.0;
	for split in t_values {
		if let Some(to_split_next) = to_split {
			let (sub_seg, _to_split) = split_path_seg(&to_split_next, (*split - prev_split) / (1.0 - prev_split));
			to_split = _to_split;
			sub_segments.push(sub_seg);
			prev_split = *split;
		} else {
			sub_segments.push(None);
		}
	}
	sub_segments.push(to_split);
	sub_segments
}

pub fn composite_boolean_operation(mut select: BooleanOperation, shapes: &mut Vec<RefCell<ShapeLayer>>) -> Result<Vec<ShapeLayer>, BooleanOperationError> {
	if select == BooleanOperation::SubtractFront {
		select = BooleanOperation::SubtractBack;
		let temp_len = shapes.len();
		shapes.swap(0, temp_len - 1);
	}
	match select {
		BooleanOperation::Union | BooleanOperation::Intersection => {
			// We must attempt to union each shape with every other shape
			let mut subject_idx = 0;
			while subject_idx < shapes.len() {
				let mut shape_idx = 0;
				while shape_idx < shapes.len() && subject_idx < shapes.len() {
					if shape_idx == subject_idx {
						shape_idx += 1;
						continue;
					}
					let partial_union = boolean_operation(select, &mut shapes[subject_idx].borrow_mut(), &mut shapes[shape_idx].borrow_mut());
					match partial_union {
						Ok(temp_union) => {
							// The result of a successful union will be exactly one shape
							if let Some(result) = temp_union.into_iter().next() {
								shapes.push(RefCell::new(result));
								shapes.swap_remove(subject_idx);
								shapes.swap_remove(shape_idx);
							} else {
								return Err(BooleanOperationError::NoResult);
							}
						}
						Err(BooleanOperationError::NothingDone) => shape_idx += 1,
						Err(err) => return Err(err),
					}
				}
				subject_idx += 1;
			}
			Ok(shapes.iter().map(|ref_shape_layer| ref_shape_layer.borrow().clone()).collect())
		}
		BooleanOperation::SubtractBack => {
			let mut result = vec![shapes[0].borrow().clone()];
			for shape_idx in shapes.iter().skip(1) {
				let mut temp = Vec::new();
				for mut partial in result {
					match boolean_operation(select, &mut partial, &mut shape_idx.borrow_mut()) {
						Ok(mut partial_result) => temp.append(&mut partial_result),
						Err(BooleanOperationError::NothingDone) => temp.push(partial),
						Err(err) => return Err(err),
					}
				}
				result = temp; // This move should be done without copying
			}
			Ok(result)
		}
		BooleanOperation::Difference => {
			let mut difference = Vec::new();
			for shape_idx in 0..shapes.len() {
				shapes.swap(0, shape_idx);
				difference.append(&mut composite_boolean_operation(BooleanOperation::SubtractBack, shapes)?);
			}
			Ok(difference)
		}
		BooleanOperation::SubtractFront => unreachable!("composite boolean operation: unreachable subtract from back"),
	}
}

// TODO: check if shapes are filled
// TODO: Bug: shape with at least two subpaths and comprised of many unions sometimes has erroneous movetos embedded in edges
pub fn boolean_operation(mut select: BooleanOperation, alpha: &mut ShapeLayer, beta: &mut ShapeLayer) -> Result<Vec<ShapeLayer>, BooleanOperationError> {
	if alpha.shape.manipulator_groups().is_empty() || beta.shape.manipulator_groups().is_empty() {
		return Err(BooleanOperationError::InvalidSelection);
	}
	if select == BooleanOperation::SubtractFront {
		select = BooleanOperation::SubtractBack;
		swap(alpha, beta);
	}
	let mut alpha_shape = close_path(&(&alpha.shape).into());
	let beta_shape = close_path(&(&beta.shape).into());
	let beta_reverse = close_path(&reverse_path(&beta_shape));
	let alpha_dir = Cycle::direction_for_path(&alpha_shape)?;
	let beta_dir = Cycle::direction_for_path(&beta_shape)?;
	match select {
		BooleanOperation::Union => {
			match if beta_dir == alpha_dir {
				PathGraph::from_paths(&alpha_shape, &beta_shape)
			} else {
				PathGraph::from_paths(&alpha_shape, &beta_reverse)
			} {
				Ok(graph) => {
					let mut cycles = graph.get_cycles();
					// "extra calls to ParamCurveArea::area here"
					let mut boolean_union = graph.get_shape(
						cycles.iter().reduce(|max, cycle| if cycle.area().abs() >= max.area().abs() { cycle } else { max }).unwrap(),
						&alpha.style,
					);
					for interior in collect_shapes(&graph, &mut cycles, |dir| dir != alpha_dir, |_| &alpha.style)? {
						//TODO: this is not very efficient or nice to read
						let mut a_path: BezPath = (&boolean_union.shape).into();
						let b_path: BezPath = (&interior.shape).into();
						add_subpath(&mut a_path, b_path);
						boolean_union.shape = a_path.iter().into();
					}
					Ok(vec![boolean_union])
				}
				Err(BooleanOperationError::NoIntersections) => {
					// If shape is inside the other the Union is just the larger
					// Check could also be done with area and single ray cast
					if cast_horizontal_ray(point_on_curve(&beta_shape), &alpha_shape) % 2 != 0 {
						Ok(vec![alpha.clone()])
					} else if cast_horizontal_ray(point_on_curve(&alpha_shape), &beta_shape) % 2 != 0 {
						beta.style = alpha.style.clone();
						Ok(vec![beta.clone()])
					} else {
						Err(BooleanOperationError::NothingDone)
					}
				}
				Err(err) => Err(err),
			}
		}
		BooleanOperation::Difference => {
			let graph = if beta_dir != alpha_dir {
				PathGraph::from_paths(&alpha_shape, &beta_shape)?
			} else {
				PathGraph::from_paths(&alpha_shape, &beta_reverse)?
			};
			collect_shapes(&graph, &mut graph.get_cycles(), |_| true, |dir| if dir == alpha_dir { &alpha.style } else { &beta.style })
		}
		BooleanOperation::Intersection => {
			match if beta_dir == alpha_dir {
				PathGraph::from_paths(&alpha_shape, &beta_shape)
			} else {
				PathGraph::from_paths(&alpha_shape, &beta_reverse)
			} {
				Ok(graph) => {
					let mut cycles = graph.get_cycles();
					// "extra calls to ParamCurveArea::area here"
					cycles.remove(
						cycles
							.iter()
							.enumerate()
							.reduce(|(max_index, max), (index, cycle)| if cycle.area().abs() >= max.area().abs() { (index, cycle) } else { (max_index, max) })
							.unwrap()
							.0,
					);
					collect_shapes(&graph, &mut cycles, |dir| dir == alpha_dir, |_| &alpha.style)
				}
				Err(BooleanOperationError::NoIntersections) => {
					// Check could also be done with area and single ray cast
					if cast_horizontal_ray(point_on_curve(&beta_shape), &alpha_shape) % 2 != 0 {
						beta.style = alpha.style.clone();
						Ok(vec![beta.clone()])
					} else if cast_horizontal_ray(point_on_curve(&alpha_shape), &beta_shape) % 2 != 0 {
						Ok(vec![alpha.clone()])
					} else {
						Err(BooleanOperationError::NothingDone)
					}
				}
				Err(err) => Err(err),
			}
		}
		BooleanOperation::SubtractFront => {
			unreachable!("Boolean operation: unreachable subtract from back");
		}
		BooleanOperation::SubtractBack => {
			match if beta_dir != alpha_dir {
				PathGraph::from_paths(&alpha_shape, &beta_shape)
			} else {
				PathGraph::from_paths(&alpha_shape, &beta_reverse)
			} {
				Ok(graph) => collect_shapes(&graph, &mut graph.get_cycles(), |dir| dir == alpha_dir, |_| &alpha.style),
				Err(BooleanOperationError::NoIntersections) => {
					if cast_horizontal_ray(point_on_curve(&beta_shape), &alpha_shape) % 2 != 0 {
						add_subpath(&mut alpha_shape, if beta_dir == alpha_dir { reverse_path(&beta_shape) } else { beta_shape });
						Ok(vec![alpha.clone()])
					} else {
						Err(BooleanOperationError::NothingDone)
					}
				}
				Err(err) => Err(err),
			}
		}
	}
}

// TODO check bounding boxes more rigorously
pub fn cast_horizontal_ray(from: Point, into: &BezPath) -> usize {
	let mut ray = PathSeg::Line(Line {
		p0: from,
		p1: Point { x: from.x + 1.0, y: from.y },
	});
	let mut intersects = Vec::new();
	for ref mut seg in into.segments() {
		if kurbo::ParamCurveExtrema::bounding_box(seg).x1 > from.x {
			line_curve_intersections((&mut ray, seg), |_, b| valid_t(b), &mut intersects);
		}
	}
	intersects.len()
}

/// Uses curve start point as point on the curve.
/// # Panics
/// This function panics if the `curve` is empty.
pub fn point_on_curve(curve: &BezPath) -> Point {
	curve.segments().next().unwrap().start()
}

/// # Panics
/// This function panics if the curve has no `PathSeg`s.
pub fn bounding_box(curve: &BezPath) -> Rect {
	curve
		.segments()
		.map(|seg| <PathSeg as ParamCurveExtrema>::bounding_box(&seg))
		.reduce(|bounds, rect| bounds.union(rect))
		.unwrap()
}

fn collect_shapes<'a, F, G>(graph: &PathGraph, cycles: &mut Vec<Cycle>, predicate: F, style: G) -> Result<Vec<ShapeLayer>, BooleanOperationError>
where
	F: Fn(Direction) -> bool,
	G: Fn(Direction) -> &'a PathStyle,
{
	let mut shapes = Vec::new();

	if cycles.is_empty() {
		return Err(BooleanOperationError::Unexpected);
	}

	for cycle in cycles {
		match cycle.direction() {
			Ok(dir) => {
				if predicate(dir) {
					shapes.push(graph.get_shape(cycle, style(dir)));
				}
			}
			// Exclude cycles with 0.0 area
			Err(_err) => (),
		}
	}
	Ok(shapes)
}

pub fn reverse_path_segment(seg: &mut PathSeg) {
	match seg {
		PathSeg::Line(line) => std::mem::swap(&mut line.p0, &mut line.p1),
		PathSeg::Quad(quad) => std::mem::swap(&mut quad.p0, &mut quad.p2),
		PathSeg::Cubic(cubic) => {
			std::mem::swap(&mut cubic.p0, &mut cubic.p3);
			std::mem::swap(&mut cubic.p1, &mut cubic.p2);
		}
	}
}

/// Reverses `path` by reversing each `PathSeg`, and reversing the order of `PathSegs` within each subpath.
/// Note: a closed path might no longer be closed after applying this function.
pub fn reverse_path(path: &BezPath) -> BezPath {
	let mut curve = Vec::new();
	let mut temp = Vec::new();
	let mut path_segments = path.segments();

	for element in path.iter() {
		match element {
			PathEl::MoveTo(_) => {
				curve.append(&mut temp.into_iter().rev().collect());
				temp = Vec::new();
			}
			_ => {
				if let Some(mut seg) = path_segments.next() {
					reverse_path_segment(&mut seg);
					temp.push(seg);
				}
			}
		}
	}
	curve.append(&mut temp.into_iter().rev().collect());
	BezPath::from_path_segments(curve.into_iter())
}

/// Close off all sub-paths in curve by inserting a `ClosePath` whenever a `MoveTo` is not preceded by one.
pub fn close_path(curve: &BezPath) -> BezPath {
	let mut new = BezPath::new();
	let mut path_closed_flag = true;
	for el in curve.iter() {
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

/// Concatenate `b` to `a`, where `b` is not a new subpath but a continuation of `a`.
pub fn concat_paths(a: &mut Vec<PathEl>, b: &BezPath) {
	if a.is_empty() {
		a.append(&mut b.elements().to_vec());
		return;
	}
	// Remove closepath
	if let Some(PathEl::ClosePath) = a.last() {
		a.remove(a.len() - 1);
	}
	// Skip initial `MoveTo`, which should be guaranteed to exist
	b.iter().skip(1).for_each(|element| a.push(element));
}

/// Concatenate `b` to `a`, where `b` is a new subpath.
pub fn add_subpath(a: &mut BezPath, b: BezPath) {
	b.into_iter().for_each(|el| a.push(el));
}

pub fn path_length(a: &BezPath, accuracy: Option<f64>) -> f64 {
	let mut sum = 0.0;
	// Computing arc length with `F64PRECISE` accuracy is probably ridiculous
	match accuracy {
		Some(val) => a.segments().for_each(|seg| sum += seg.arclen(val)),
		None => a.segments().for_each(|seg| sum += seg.arclen(F64PRECISE)),
	}
	sum
}

pub fn path_area(a: &BezPath) -> f64 {
	a.segments().fold(0.0, |mut area, seg| {
		area += seg.signed_area();
		area
	})
}
