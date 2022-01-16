use crate::{
	color::Color,
	consts::F64PRECISION,
	intersection::{intersections, Intersect, Origin},
	layers::{
		simple_shape::Shape,
		style::{Fill, PathStyle, Stroke},
	},
};
use kurbo::{BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveArea, ParamCurveExtrema, PathEl, PathSeg, QuadBez, Rect};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter};
use std::{fmt, ops::Not}; // are using fmt::Result, but don't want to conlict with std::result::Result

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
		if vertex == self.vertices[0].0 {
			return true;
		}
		return false;
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

	/// - if the path has multiple sub paths the function Errs
	/// - if the path is empty (has no segments) the function Errs
	/// - if the path crosses itself the computed direction may be (probably will be) wrong, on account of it not really being defined
	/// - the path does not need to end in a ClosePath, however if it doesn't, the final vertex must compare exactly equal to the start vertex.
	///   Which, with floating point precision, is unlikely.
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
#[allow(dead_code)] //<---- remove this @ release
impl PathGraph {
	pub fn from_paths(alpha: &BezPath, beta: &BezPath, reverse: bool) -> Result<PathGraph, BooleanOperationError> {
		//TODO: check for closed paths somewhere, maybe here?
		let mut new = PathGraph {
			vertices: intersections(alpha, beta).into_iter().map(|i| Vertex { intersect: i, edges: Vec::new() }).collect(),
		};
		// we only consider graphs with even numbers of intersections.
		// An odd number of intersections occurrs when either
		//    1. There exists a tangential intersection (which shouldn't effect boolean operations)
		//    2. The algorithm has found an extra intersection or missed an intersection
		if new.size() == 0 || new.size() % 2 != 0 {
			return Err(BooleanOperationError::InvalidIntersections);
		}
		new.add_edges_from_path(alpha, Origin::Alpha, false);
		new.add_edges_from_path(beta, Origin::Beta, reverse);
		// log::debug!("size: {}, {:?}", new.size(), new);
		Ok(new)
	}

	/// Behavior: path should be split at intersection point, not intersection t value, in case of discrepancy between paths
	///   - implementing this behavior may not be feasible, instead reduce discrepancies
	/// TODO: This function panics if an time value is NAN, no time value should ever be NAN, but this case should be handled, maybe not here
	/// NOTE: about intersection time_val order
	fn add_edges_from_path(&mut self, path: &BezPath, origin: Origin, reverse: bool) {
		let mut seg_idx = 0;
		//cstart holds the idx of the vertex the current edge is starting from
		let mut cstart = None;
		let mut current = Vec::new();
		// in order to iterate through once, store information for incomplete first edge
		let mut beginning = Vec::new();
		let mut start_idx = None;
		#[allow(clippy::explicit_counter_loop)]
		for seg in path.segments() {
			let mut intersects = self.intersects_in_seg(seg_idx, origin);
			if intersects.len() > 0 {
				intersects.sort_by(|(_, t1), (_, t2)| t1.partial_cmp(t2).unwrap());
				for (vertex_id, t_val) in intersects {
					let (seg1, seg2) = split_path_seg(&seg, t_val);
					match cstart {
						Some(idx) => {
							do_if!(seg1, end_of_edge { current.push(end_of_edge)});
							self.add_edge(origin, idx, vertex_id, current, reverse);
							cstart = Some(vertex_id);
							current = Vec::new();
							do_if!(seg2, start_of_edge { current.push(start_of_edge)});
						}
						None => {
							cstart = Some(vertex_id);
							start_idx = Some(vertex_id);
							do_if!(seg1, end_of_begining {beginning.push(end_of_begining)});
							do_if!(seg2, start_of_edge {current.push(start_of_edge)});
						}
					}
				}
			} else {
				match cstart {
					Some(_) => current.push(seg),
					None => beginning.push(seg),
				}
			}
			seg_idx += 1;
		}
		current.append(&mut beginning);
		self.add_edge(origin, cstart.unwrap(), start_idx.unwrap(), current, reverse);
	}

	fn add_edge(&mut self, origin: Origin, vertex: usize, destination: usize, mut curve: Vec<PathSeg>, reverse: bool) {
		if reverse {
			for seg in &mut curve {
				reverse_pathseg(seg);
			}
		}
		let mut new_edge = Edge {
			from: origin,
			destination,
			curve: BezPath::from_path_segments(curve.into_iter()),
		};
		if reverse {
			new_edge.destination = vertex;
			self.vertices[destination].edges.push(new_edge);
		} else {
			self.vertices[vertex].edges.push(new_edge);
		}
	}

	/// returns all intersects in segment with seg_idx from origin
	fn intersects_in_seg(&self, seg_idx: usize, origin: Origin) -> Vec<(usize, f64)> {
		self.vertices
			.iter()
			.enumerate()
			.filter_map(|(v_idx, vertex)| {
				if vertex.intersect.seg_idx(origin) == seg_idx {
					Some((v_idx, vertex.intersect.t_val(origin)))
				} else {
					None
				}
			})
			.collect()
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

	// return reference to intersect associated with the vertex at idx
	pub fn intersect(&self, idx: usize) -> &Intersect {
		&self.vertices[idx].intersect
	}

	/// where a valid cycle alternates edge Origin
	fn get_cycle(&self, cycle: &mut Cycle, marker_map: &mut Vec<u8>) {
		marker_map[cycle.prev_vertex()] += 1;
		let next_edge = self.vertex(cycle.prev_vertex()).edges.iter().find(|edge| edge.from != cycle.prev_edge_origin()).unwrap();
		if !cycle.extend(next_edge.destination, next_edge.from, &next_edge.curve) {
			return self.get_cycle(cycle, marker_map);
		}
	}

	pub fn get_cycles(&self) -> Vec<Cycle> {
		let mut cycles = Vec::new();
		let mut markers = Vec::new();
		markers.resize(self.size(), 0);
		self.vertices.iter().enumerate().for_each(|(vertex_idx, _vertex)| {
			if markers[vertex_idx] < 2 {
				let mut temp = Cycle::new(vertex_idx, Origin::Alpha);
				self.get_cycle(&mut temp, &mut markers);
				cycles.push(temp);
				temp = Cycle::new(vertex_idx, Origin::Beta);
				self.get_cycle(&mut temp, &mut markers);
				cycles.push(temp);
			}
		});
		cycles
	}

	pub fn get_shape(&self, cycle: &Cycle) -> Shape {
		let mut path = BezPath::new();
		let vertices = cycle.vertices();
		for idx in 1..vertices.len() {
			// we expect the cycle to be valid, this should not panic
			concat_paths(&mut path, &self.edge(vertices[idx - 1].0, vertices[idx].0, vertices[idx].1).unwrap().curve);
		}
		Shape::from_bez_path(path, PathStyle::new(Some(Stroke::new(Color::BLACK, 1.0)), Some(Fill::none())), false)
	}
}

/// if t is on (0, 1), returns the split path
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

pub fn sub_path_seg(p: &PathSeg, mut t1: f64, mut t2: f64) -> (Option<PathSeg>, Option<PathSeg>, Option<PathSeg>) {
	if t1 > t2 {
		std::mem::swap(&mut t1, &mut t2);
	}
	let (p1, unhewn) = split_path_seg(p, t1);
	let (p2, p3) = if let Some(unhewn_seg) = unhewn {
		let t2_in_unhewn = (t2 - t1) / (1.0 - t1);
		split_path_seg(&unhewn_seg, t2_in_unhewn)
	} else {
		(None, None)
	};
	(p1, p2, p3)
}

/// TODO: For the Union and intersection operations, what should the new Fill and Stroke be? --> see document.rs
pub fn boolean_operation(select: BooleanOperation, alpha: &Shape, beta: &Shape) -> Result<Vec<Shape>, BooleanOperationError> {
	let alpha = &alpha.path;
	let beta = &beta.path;
	if alpha.is_empty() || beta.is_empty() {
		return Err(BooleanOperationError::InvalidSelection);
	}
	let alpha_dir = Cycle::direction_for_path(&alpha)?;
	let beta_dir = Cycle::direction_for_path(&beta)?;
	log::debug!("alpha: {:?} beta: {:?}", alpha_dir, beta_dir);
	match select {
		BooleanOperation::Union => {
			let graph = PathGraph::from_paths(&alpha, &beta, alpha_dir != beta_dir)?;
			let mut cycles = graph.get_cycles();
			// "extra calls to ParamCurveArea::area here"
			let outline: Cycle = (*cycles.iter().reduce(|max, cycle| if cycle.area().abs() >= max.area().abs() { cycle } else { max }).unwrap()).clone();
			let mut insides = collect_shapes(&graph, &mut cycles, |dir| dir != alpha_dir)?;
			insides.push(graph.get_shape(&outline));
			Ok(insides)
		}
		BooleanOperation::Difference => {
			let graph = PathGraph::from_paths(&alpha, &beta, alpha_dir == beta_dir)?;
			collect_shapes(&graph, &mut graph.get_cycles(), |_| true)
		}
		BooleanOperation::Intersection => {
			let graph = PathGraph::from_paths(&alpha, &beta, alpha_dir != beta_dir)?;
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
			collect_shapes(&graph, &mut cycles, |dir| dir == alpha_dir)
		}
		BooleanOperation::SubtractBack => {
			let graph = PathGraph::from_paths(&alpha, &beta, alpha_dir == beta_dir)?;
			collect_shapes(&graph, &mut graph.get_cycles(), |dir| dir != alpha_dir)
		}
		BooleanOperation::SubtractFront => {
			let graph = PathGraph::from_paths(&alpha, &beta, alpha_dir == beta_dir)?;
			collect_shapes(&graph, &mut graph.get_cycles(), |dir| dir == alpha_dir)
		}
	}
}

/// panics if the curve has no PathSeg's
pub fn bounding_box(curve: &BezPath) -> Rect {
	curve
		.segments()
		.map(|seg| <PathSeg as ParamCurveExtrema>::bounding_box(&seg))
		.reduce(|bounds, rect| bounds.union(rect))
		.unwrap()
}

fn collect_shapes<F>(graph: &PathGraph, cycles: &mut Vec<Cycle>, predicate: F) -> Result<Vec<Shape>, BooleanOperationError>
where
	F: Fn(Direction) -> bool,
{
	let mut shapes = Vec::new();
	if cycles.len() == 0 {
		return Err(BooleanOperationError::Unexpected);
	}
	for cycle in cycles {
		match cycle.direction() {
			Ok(dir) => {
				log::debug!("dir: {:?}", dir);
				if predicate(dir) {
					shapes.push(graph.get_shape(cycle));
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

pub fn is_closed(curve: &BezPath) -> bool {
	curve.iter().last() == Some(PathEl::ClosePath)
}

pub fn concat_paths(a: &mut BezPath, b: &BezPath) {
	b.iter().for_each(|element| a.push(element));
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
