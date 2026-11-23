//! The areas of a laid out node that react to the pointer, in node graph pixels. A hit test scans each node's reach, a box around
//! everything the node draws, and builds the exact parts of only the nodes it lands in.

use crate::consts::{GRID_SIZE, HALF_GRID_SIZE};
use glam::{DVec2, IVec2};
use graph_layout::{CHAIN_WIDTH, Node, NodeKind};

const GRID: f64 = GRID_SIZE as f64;
const HALF_GRID: f64 = HALF_GRID_SIZE as f64;
/// How far a port on a node's side reaches outward, away from the node, and inward over it.
const SIDE_PORT_OUTWARD_REACH: f64 = 12.;
const SIDE_PORT_INWARD_REACH: f64 = 8.;
const SIDE_PORT_HALF_HEIGHT: f64 = 12.;
const STACK_PORT_HALF_SIZE: DVec2 = DVec2::new(12., 8.);
/// A waypoint's body also covers the gutters beside it, so a press near it moves it unless it lands on a port.
const WAYPOINT_HALF_SIZE: DVec2 = DVec2::new(15., 8.);
/// A waypoint's wires attach at its center but are grabbed from a round gutter on either side of it.
const WAYPOINT_GUTTER_OFFSET: f64 = 10.;
const WAYPOINT_GUTTER_RADIUS: f64 = 5.;
const GRIP_WIDTH: f64 = 8.;
const NAME_LEFT_OFFSET: f64 = HALF_GRID + 3. * GRID + 8.;

/// What a node draws beyond the size the layout gives it.
#[derive(Clone, Copy, Debug, Default)]
pub struct NodeShape {
	/// How many inputs have a port, which doesn't count a hidden primary input.
	pub inputs: u32,
	pub outputs: u32,
	/// Whether a node's primary input has no port, though its row is still drawn.
	pub primary_input_hidden: bool,
	pub primary_output_hidden: bool,
	/// The width of a layer's name in pixels.
	pub name_width: f64,
	pub locked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
	Body,
	/// The input drawn in this grid row of a node, or a layer's stack input at 0 and side input at 1.
	Input(u32),
	Output(u32),
	Visibility,
	Lock,
	Grip,
	Name,
}

/// A rounded rectangle of a node that reacts to the pointer.
#[derive(Clone, Copy, Debug)]
pub struct PartTarget {
	pub min: DVec2,
	pub max: DVec2,
	pub corner_radius: f64,
	pub part: Part,
}

impl PartTarget {
	fn around(center: DVec2, half_size: DVec2, corner_radius: f64, part: Part) -> Self {
		Self {
			min: center - half_size,
			max: center + half_size,
			corner_radius,
			part,
		}
	}

	/// A port on the left side of a node, reaching further out than over the node.
	fn input_port(center: DVec2, input: u32) -> Self {
		Self {
			min: center - DVec2::new(SIDE_PORT_OUTWARD_REACH, SIDE_PORT_HALF_HEIGHT),
			max: center + DVec2::new(SIDE_PORT_INWARD_REACH, SIDE_PORT_HALF_HEIGHT),
			corner_radius: 4.,
			part: Part::Input(input),
		}
	}

	/// A port on the right side of a node, reaching further out than over the node.
	fn output_port(center: DVec2, output: u32) -> Self {
		Self {
			min: center - DVec2::new(SIDE_PORT_INWARD_REACH, SIDE_PORT_HALF_HEIGHT),
			max: center + DVec2::new(SIDE_PORT_OUTWARD_REACH, SIDE_PORT_HALF_HEIGHT),
			corner_radius: 4.,
			part: Part::Output(output),
		}
	}

	/// Whether the rounded rectangle shares any area with the box from `min` to `max`.
	pub fn overlaps(&self, min: DVec2, max: DVec2) -> bool {
		if !min.cmplt(self.max).all() || !self.min.cmplt(max).all() {
			return false;
		}

		let radius = self.clamped_radius();
		let gap = (self.min + radius - max).max(min - (self.max - radius)).max(DVec2::ZERO);
		gap == DVec2::ZERO || gap.length_squared() < radius * radius
	}

	/// Whether `point` is inside the rounded rectangle.
	pub fn contains(&self, point: DVec2) -> bool {
		if !point.cmpge(self.min).all() || !point.cmplt(self.max).all() {
			return false;
		}

		let radius = self.clamped_radius();
		let inner = point.clamp(self.min + radius, self.max - radius);
		point.distance_squared(inner) <= radius * radius
	}

	fn clamped_radius(&self) -> f64 {
		self.corner_radius.min((self.max - self.min).min_element() / 2.)
	}
}

/// A node as drawn at `position`, in grid cells, with the chain it is drawn beside if it is a layer.
#[derive(Clone, Copy, Debug)]
pub struct PlacedNode<'a> {
	pub node: &'a Node,
	pub shape: &'a NodeShape,
	pub position: IVec2,
	pub chain_length: u32,
}

impl PlacedNode<'_> {
	/// The body: a layer with its chain span, a node below its top connector strip, or the pill around a waypoint.
	pub fn body(&self) -> PartTarget {
		let top_left = self.top_left();
		let size = self.node.size.as_dvec2() * GRID;

		match self.node.kind {
			NodeKind::Waypoint => PartTarget::around(top_left + DVec2::new(0., GRID), WAYPOINT_HALF_SIZE, WAYPOINT_HALF_SIZE.y, Part::Body),
			NodeKind::Layer => PartTarget {
				min: top_left - DVec2::new(self.chain_width(), 0.),
				max: top_left + size,
				corner_radius: 10.,
				part: Part::Body,
			},
			NodeKind::Node => {
				let body_top_left = top_left + DVec2::new(0., HALF_GRID);
				PartTarget {
					min: body_top_left,
					max: body_top_left + size - DVec2::new(0., GRID),
					corner_radius: 3.,
					part: Part::Body,
				}
			}
		}
	}

	/// Calls `visit` with each part, the body last.
	pub fn parts(&self, mut visit: impl FnMut(PartTarget)) {
		match self.node.kind {
			NodeKind::Waypoint => self.waypoint_ports(&mut visit),
			NodeKind::Layer => self.layer_parts(&mut visit),
			NodeKind::Node => self.node_ports(&mut visit),
		}
		visit(self.body());
	}

	fn waypoint_ports(&self, visit: &mut impl FnMut(PartTarget)) {
		let center = self.top_left() + DVec2::new(0., GRID);
		let gutter = |x_offset: f64, part| PartTarget::around(center + DVec2::new(x_offset, 0.), DVec2::splat(WAYPOINT_GUTTER_RADIUS), WAYPOINT_GUTTER_RADIUS, part);
		visit(gutter(-WAYPOINT_GUTTER_OFFSET, Part::Input(0)));
		visit(gutter(WAYPOINT_GUTTER_OFFSET, Part::Output(0)));
	}

	fn node_ports(&self, visit: &mut impl FnMut(PartTarget)) {
		let top_left = self.top_left();
		let width = self.node.size.x as f64 * GRID;
		let row_center = |row: u32| GRID * (row as f64 + 1.);

		let first_input_row = u32::from(self.shape.primary_input_hidden);
		for row in first_input_row..first_input_row + self.shape.inputs {
			visit(PartTarget::input_port(top_left + DVec2::new(0., row_center(row)), row));
		}
		for output in u32::from(self.shape.primary_output_hidden)..self.shape.outputs {
			visit(PartTarget::output_port(top_left + DVec2::new(width, row_center(output)), output));
		}
	}

	fn layer_parts(&self, visit: &mut impl FnMut(PartTarget)) {
		let top_left = self.top_left();
		let size = self.node.size.as_dvec2() * GRID;

		visit(PartTarget::around(top_left + DVec2::new(2. * GRID, size.y + 8.), STACK_PORT_HALF_SIZE, 4., Part::Input(0)));
		if self.has_side_input() {
			visit(PartTarget::input_port(top_left + DVec2::new(0., GRID), 1));
		}
		visit(PartTarget::around(top_left + DVec2::new(2. * GRID, -8.), STACK_PORT_HALF_SIZE, 4., Part::Output(0)));

		let icon_half_size = DVec2::splat(HALF_GRID);
		visit(PartTarget::around(top_left + DVec2::new(size.x, GRID), icon_half_size, 3., Part::Visibility));
		if self.shape.locked {
			visit(PartTarget::around(top_left + DVec2::new(size.x - GRID, GRID), icon_half_size, 3., Part::Lock));
		}

		let icons_width = if self.shape.locked { GRID } else { 0. };
		let grip_right = top_left.x + size.x - HALF_GRID - icons_width;
		let handle_top = top_left.y + HALF_GRID;
		let handle_bottom = top_left.y + GRID + HALF_GRID;
		visit(PartTarget {
			min: DVec2::new(grip_right - GRIP_WIDTH, handle_top),
			max: DVec2::new(grip_right, handle_bottom),
			corner_radius: 0.,
			part: Part::Grip,
		});

		let name_left = top_left.x + NAME_LEFT_OFFSET;
		let name_right = (name_left + self.shape.name_width).min(grip_right - GRIP_WIDTH);
		if name_right > name_left {
			visit(PartTarget {
				min: DVec2::new(name_left, handle_top),
				max: DVec2::new(name_right, handle_bottom),
				corner_radius: 3.,
				part: Part::Name,
			});
		}
	}

	fn top_left(&self) -> DVec2 {
		self.position.as_dvec2() * GRID
	}

	fn has_side_input(&self) -> bool {
		self.shape.inputs > 1
	}

	fn chain_width(&self) -> f64 {
		if self.has_side_input() { (self.chain_length as i32 * CHAIN_WIDTH + 1) as f64 * GRID } else { 0. }
	}
}
