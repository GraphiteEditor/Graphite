use crate::message_prelude::Message;

pub use super::layer_panel::{layer_panel_entry, LayerMetadata, LayerPanelEntry, RawBuffer};
use super::{DocumentMessage, DocumentMessageHandler};

use graphene::layers::layer_info::LayerDataType;
use graphene::layers::simple_shape::Shape;
use graphene::LayerId;
use graphene::{document::Document as GrapheneDocument, Operation};

use glam::{DAffine2, DVec2};
use kurbo::{BezPath, PathSeg};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

pub type DocumentSave = (GrapheneDocument, HashMap<Vec<LayerId>, LayerMetadata>);

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum FlipAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
	Average,
}

#[derive(PartialEq, Clone, Debug)]
pub enum VectorManipulatorSegment {
	Line(DVec2, DVec2),
	Quad(DVec2, DVec2, DVec2),
	Cubic(DVec2, DVec2, DVec2, DVec2),
}

#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorManipulatorShape {
	/// The path to the layer
	pub layer_path: Vec<LayerId>,
	/// The outline of the shape
	pub path: kurbo::BezPath,
	/// The segments containing the control points / manipulator handles
	pub segments: Vec<VectorManipulatorSegment>,
	/// The control points / manipulator handles
	pub anchors: Vec<VectorManipulatorAnchor>,
	/// The compound Bezier curve is closed
	pub closed: bool,
	/// The transformation matrix to apply
	pub transform: DAffine2,
}

impl VectorManipulatorShape {
	// TODO: Figure out a more elegant way to construct this
	pub fn new(layer_path: Vec<LayerId>, transform: DAffine2, shape: &Shape) -> Self {
		let mut manipulator_shape = VectorManipulatorShape {
			layer_path,
			path: shape.path.clone(),
			closed: shape.closed,
			transform,
			segments: vec![],
			anchors: vec![],
		};
		manipulator_shape.segments = manipulator_shape.create_segments_from_kurbo();
		manipulator_shape.anchors = manipulator_shape.create_anchors_from_kurbo();
		manipulator_shape
	}

	/// Place points in local space
	fn to_local_space(&self, point: kurbo::Point) -> DVec2 {
		self.transform.transform_point2(DVec2::from((point.x, point.y)))
	}

	/// Create the anchors from the kurbo path, only done on construction
	fn create_anchors_from_kurbo(&self) -> Vec<VectorManipulatorAnchor> {
		type IndexedEl = (usize, kurbo::PathEl);

		// Create an anchor on the boundary between two kurbo PathElements with optional handles
		let create_anchor_manipulator = |first: IndexedEl, second: IndexedEl| -> VectorManipulatorAnchor {
			let mut handle1 = None;
			let mut anchor_position: glam::DVec2 = glam::DVec2::ZERO;
			let mut handle2 = None;
			let (first_id, first_element) = first;
			let (second_id, second_element) = second;

			match first_element {
				kurbo::PathEl::MoveTo(anchor) | kurbo::PathEl::LineTo(anchor) => anchor_position = self.to_local_space(anchor),
				kurbo::PathEl::QuadTo(handle, anchor) | kurbo::PathEl::CurveTo(_, handle, anchor) => {
					anchor_position = self.to_local_space(anchor);
					handle1 = Some(VectorManipulatorPoint {
						element_id: first_id,
						position: self.to_local_space(handle),
					});
				}
				_ => (),
			}

			match second_element {
				kurbo::PathEl::CurveTo(handle, _, _) | kurbo::PathEl::QuadTo(handle, _) => {
					handle2 = Some(VectorManipulatorPoint {
						element_id: second_id,
						position: self.to_local_space(handle),
					});
				}
				_ => (),
			}

			VectorManipulatorAnchor {
				point: VectorManipulatorPoint {
					element_id: first_id,
					position: anchor_position,
				},
				close_element_id: None,
				handles: (handle1, handle2),
				handle_mirroring: true,
			}
		};

		// We need the indices paired with the kurbo path elements
		let indexed_elements = self.path.elements().iter().enumerate().map(|(index, element)| (index, *element)).collect::<Vec<IndexedEl>>();

		// Create the manipulation points
		let mut points: Vec<VectorManipulatorAnchor> = vec![];
		let (mut first, mut last): (Option<IndexedEl>, Option<IndexedEl>) = (None, None);
		let mut close_element_id: Option<usize> = None;

		// Create an anchor at each join between two kurbo segments
		for elements in indexed_elements.windows(2) {
			let (current_index, current_element) = elements[0];
			let (_, next_element) = elements[1];

			// An anchor cannot stradle a line / curve segment and a ClosePath segment
			if matches!(next_element, kurbo::PathEl::ClosePath) {
				break;
			}

			// TODO: Currently a unique case for [MoveTo, CurveTo, ...], refactor more generally if possible
			if matches!(current_element, kurbo::PathEl::MoveTo(_)) && (matches!(next_element, kurbo::PathEl::CurveTo(_, _, _)) || matches!(next_element, kurbo::PathEl::QuadTo(_, _))) {
				close_element_id = Some(current_index);
				continue;
			}

			// Keep track of the first and last elements of this shape
			if first.is_none() {
				first = Some(elements[0]);
			}
			last = Some(elements[1]);

			points.push(create_anchor_manipulator(elements[0], elements[1]));
		}

		// Close the shape
		if let (Some(first), Some(last)) = (first, last) {
			let mut element = create_anchor_manipulator(last, first);
			element.close_element_id = close_element_id;
			points.push(element);
		}

		points
	}

	/// Update the anchors to natch the kurbo path
	fn update_anchors(&mut self, path: &BezPath) {
		for anchor_index in 0..self.anchors.len() {
			let elements = path.elements();
			let (h1, h2) = self.anchors[anchor_index].handles;
			match elements[self.anchors[anchor_index].point.element_id] {
				kurbo::PathEl::MoveTo(anchor_position) | kurbo::PathEl::LineTo(anchor_position) => self.anchors[anchor_index].point.position = self.to_local_space(anchor_position),
				kurbo::PathEl::QuadTo(handle_position, anchor_position) | kurbo::PathEl::CurveTo(_, handle_position, anchor_position) => {
					self.anchors[anchor_index].point.position = self.to_local_space(anchor_position);
					if let Some(mut handle) = h1 {
						handle.position = self.to_local_space(handle_position);
						self.anchors[anchor_index].handles.0 = Some(handle.clone());
					}
				}
				_ => (),
			}
			if let Some(handle) = h2 {
				match elements[handle.element_id] {
					kurbo::PathEl::CurveTo(handle_position, _, _) | kurbo::PathEl::QuadTo(handle_position, _) => {
						if let Some(mut handle) = h2 {
							handle.position = self.to_local_space(handle_position);
							self.anchors[anchor_index].handles.1 = Some(handle);
						}
					}
					_ => (),
				}
			}
		}
	}

	/// Create the segments from the kurbo shape
	fn create_segments_from_kurbo(&self) -> Vec<VectorManipulatorSegment> {
		self.path
			.segments()
			.map(|segment| -> VectorManipulatorSegment {
				match segment {
					PathSeg::Line(line) => VectorManipulatorSegment::Line(self.to_local_space(line.p0), self.to_local_space(line.p1)),
					PathSeg::Quad(quad) => VectorManipulatorSegment::Quad(self.to_local_space(quad.p0), self.to_local_space(quad.p1), self.to_local_space(quad.p2)),
					PathSeg::Cubic(cubic) => VectorManipulatorSegment::Cubic(
						self.to_local_space(cubic.p0),
						self.to_local_space(cubic.p1),
						self.to_local_space(cubic.p2),
						self.to_local_space(cubic.p3),
					),
				}
			})
			.collect::<Vec<VectorManipulatorSegment>>()
	}

	/// Update the segments to match the kurbo shape
	fn update_segments(&mut self, path: &BezPath) {
		path.segments().enumerate().for_each(|(index, segment)| {
			self.segments[index] = match segment {
				PathSeg::Line(line) => VectorManipulatorSegment::Line(self.to_local_space(line.p0), self.to_local_space(line.p1)),
				PathSeg::Quad(quad) => VectorManipulatorSegment::Quad(self.to_local_space(quad.p0), self.to_local_space(quad.p1), self.to_local_space(quad.p2)),
				PathSeg::Cubic(cubic) => VectorManipulatorSegment::Cubic(
					self.to_local_space(cubic.p0),
					self.to_local_space(cubic.p1),
					self.to_local_space(cubic.p2),
					self.to_local_space(cubic.p3),
				),
			};
		});
	}

	/// Update the anchors and segments to match the kurbo shape
	pub fn update_shape(&mut self, document: &DocumentMessageHandler) {
		let viewport_transform = document.graphene_document.generate_transform_relative_to_viewport(&self.layer_path).unwrap();
		let layer = document.graphene_document.layer(&self.layer_path).unwrap();
		if let LayerDataType::Shape(shape) = &layer.data {
			let path = shape.path.clone();
			self.transform = viewport_transform;

			// Update point positions
			self.update_anchors(&path);

			// Update the segment positions
			self.update_segments(&path);

			self.path = path;
		}
	}
}

#[derive(PartialEq, Clone, Debug, Copy, Default)]
pub struct VectorManipulatorPoint {
	// The associated position in the BezPath
	pub element_id: usize,
	// The sibling element if this is a handle
	pub position: glam::DVec2,
}

#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorManipulatorAnchor {
	// The associated position in the BezPath
	pub point: VectorManipulatorPoint,
	// Does this anchor point have a path close element we also needs to move?
	pub close_element_id: Option<usize>,
	// Should we mirror the handles
	pub handle_mirroring: bool,
	// Anchor handles
	pub handles: (Option<VectorManipulatorPoint>, Option<VectorManipulatorPoint>),
}

impl VectorManipulatorAnchor {
	pub fn closest_handle_or_anchor(&self, target: glam::DVec2) -> &VectorManipulatorPoint {
		let mut closest_point: &VectorManipulatorPoint = &self.point;
		let mut distance = self.point.position.distance_squared(target);
		let (handle1, handle2) = &self.handles;
		if let Some(handle1) = handle1 {
			let handle1_dist = handle1.position.distance_squared(target);
			if distance > handle1_dist {
				distance = handle1_dist;
				closest_point = handle1;
			}
		}

		if let Some(handle2) = handle2 {
			let handle2_dist = handle2.position.distance_squared(target);
			if distance > handle2_dist {
				closest_point = handle2;
			}
		}

		closest_point
	}

	/// Angle between handles in radians
	pub fn angle_between_handles(&self) -> f64 {
		if let (Some(h1), Some(h2)) = &self.handles {
			return (self.point.position - h1.position).angle_between(self.point.position - h2.position);
		}
		0.0
	}

	pub fn opposing_handle(&self, handle: &VectorManipulatorPoint) -> &Option<VectorManipulatorPoint> {
		if Some(handle) == self.handles.0.as_ref() {
			&self.handles.1
		} else {
			&self.handles.0
		}
	}
}

impl VectorManipulatorPoint {
	pub(crate) fn clone(&self) -> VectorManipulatorPoint {
		VectorManipulatorPoint {
			element_id: self.element_id,
			position: self.position,
		}
	}
}

/// Simple implementation of an pooler for overlays
/// TODO Write tests for the OverlayPooler
type OverlayGenerator = Option<Box<dyn Fn(&mut VecDeque<Message>) -> Vec<LayerId>>>;
type Channel = usize;
#[derive(Default)]
struct OverlayPool {
	overlays: Vec<Vec<LayerId>>,
	high_water_mark: usize,
	capacity: usize,
	create_overlay: OverlayGenerator,
}

impl OverlayPool {
	/// Consume an overlay and return it along with its index
	pub fn create(&mut self, responses: &mut VecDeque<Message>) -> (Vec<LayerId>, usize) {
		if self.high_water_mark >= self.overlays.len() {
			self.capacity += 1;
			self.grow_pool(responses);
		}

		let overlay: (Vec<LayerId>, usize) = (self.overlays[self.high_water_mark].clone(), self.high_water_mark);
		self.high_water_mark += 1;
		overlay
	}

	/// Recycle the pool such that it can be reused without new allocations
	/// This will not hide in use overlays
	pub fn recycle(&mut self) {
		self.high_water_mark = 0;
	}

	/// Read a single value from the pool without consuming that value
	pub fn read(&self, index: usize) -> Vec<LayerId> {
		if index >= self.high_water_mark {
			return vec![];
		}

		self.overlays[index].clone()
	}

	/// Hide the unused overlays
	pub fn hide_unused(&mut self, responses: &mut VecDeque<Message>) {
		for i in self.high_water_mark..self.capacity {
			let marker = self.overlays[i].clone();
			responses.push_back(DocumentMessage::Overlays(Operation::SetLayerVisibility { path: marker, visible: false }.into()).into());
		}
	}

	/// Remove all overlays
	pub fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		while let Some(layer) = self.overlays.pop() {
			responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: layer }.into()).into());
		}
	}

	/// Grow the pool's capacity and create new overlays
	/// This is called when the pool is full and a new overlay is requested
	pub fn grow_pool(&mut self, responses: &mut VecDeque<Message>) {
		if self.overlays.len() < self.capacity {
			let additional = self.capacity - self.overlays.len();

			self.overlays.reserve(additional);
			if let Some(create_overlay) = &self.create_overlay {
				for _ in 0..additional {
					let marker = create_overlay(responses);
					self.overlays.push(marker);
				}
			}
		}
	}
}

#[derive(Default)]
pub struct OverlayPooler {
	overlay_pools: HashMap<Channel, OverlayPool>,
}

impl OverlayPooler {
	// Add a new overlay pool with a channel id and a starting capacity
	pub fn add_overlay_pool<F>(&mut self, channel: Channel, capacity: usize, responses: &mut VecDeque<Message>, create_overlay: F)
	where
		F: Fn(&mut VecDeque<Message>) -> Vec<LayerId> + 'static,
	{
		let mut pool = OverlayPool {
			overlays: Vec::new(),
			high_water_mark: 0,
			capacity,
			create_overlay: Some(Box::new(create_overlay)),
		};
		pool.grow_pool(responses);
		self.overlay_pools.insert(channel, pool);
	}

	// Keep all the pooled overlays but recycle them for a new generation of overlays
	pub fn recycle_all_channels(&mut self) {
		for (_, pool) in self.overlay_pools.iter_mut() {
			pool.recycle();
		}
	}

	// Read a single value from the pool with the given channel id
	pub fn read_from_channel(&self, channel: Channel, index: usize) -> Vec<LayerId> {
		self.overlay_pools.get(&channel).map(|pool| pool.read(index)).unwrap_or_default()
	}

	// Consume a free overlay in the provided channel
	pub fn create_from_channel(&mut self, channel: Channel, responses: &mut VecDeque<Message>) -> (Vec<LayerId>, usize) {
		let pool = self.overlay_pools.get_mut(&channel).unwrap();
		pool.create(responses)
	}

	// Hide the remaining pooled overlays
	pub fn hide_all_extras(&mut self, responses: &mut VecDeque<Message>) {
		for pool in self.overlay_pools.values_mut() {
			pool.hide_unused(responses);
		}
	}

	// Remove all pooled overlays
	pub fn cleanup_all_channels(&mut self, responses: &mut VecDeque<Message>) {
		for pool in self.overlay_pools.values_mut() {
			pool.cleanup(responses);
		}
	}
}
