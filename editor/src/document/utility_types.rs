use crate::message_prelude::Message;

pub use super::layer_panel::{layer_panel_entry, LayerMetadata, LayerPanelEntry, RawBuffer};
use super::DocumentMessage;

use graphene::LayerId;
use graphene::{document::Document as GrapheneDocument, Operation};

use glam::{DAffine2, DVec2};
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
	pub points: Vec<VectorManipulatorAnchor>,
	/// The compound Bezier curve is closed
	pub closed: bool,
	/// The transformation matrix to apply
	pub transform: DAffine2,
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
	pub fn consume(&mut self, responses: &mut VecDeque<Message>) -> (Vec<LayerId>, usize) {
		let overlay: (Vec<LayerId>, usize) = (self.overlays[self.high_water_mark].clone(), self.high_water_mark);
		self.high_water_mark += 1;
		if self.high_water_mark >= self.capacity {
			self.grow(responses)
		}
		overlay
	}

	/// Recycle the pool such that it can be reused without new allocations
	pub fn recycle(&mut self) {
		self.high_water_mark = 0;
	}

	/// Read a single value from the pool without consuming a new slot
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

	/// Grow the pool's capacity by 2x and create new overlays
	/// This is called when the pool is full and a new overlay is requested
	pub fn grow(&mut self, responses: &mut VecDeque<Message>) {
		self.capacity += self.high_water_mark * 2;
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
		pool.grow(responses);
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
	pub fn consume_from_channel(&mut self, channel: Channel, responses: &mut VecDeque<Message>) -> (Vec<LayerId>, usize) {
		let pool = self.overlay_pools.get_mut(&channel).unwrap();
		pool.consume(responses)
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
