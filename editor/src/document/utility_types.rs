use crate::message_prelude::Message;

pub use super::layer_panel::{layer_panel_entry, LayerMetadata, LayerPanelEntry, RawBuffer};
use super::DocumentMessage;

use graphene::LayerId;
use graphene::{document::Document as GrapheneDocument, Operation};

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
	// Example of how you might create an enum for the channel
	// enum OverlayPoolType {
	// 	Shape = 0,
	// 	Anchor = 1,
	// 	Handle = 2,
	// 	HandleLine = 3,
	// }
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

// Potentially useful for pooling
// fn calculate_total_overlays_per_type(shapes: &[VectorManipulatorShape]) -> (usize, usize, usize) {
// 	shapes.iter().fold((0, 0, 0), |acc, shape| {
// 		let counts = calculate_shape_overlays_per_type(shape);
// 		(acc.0 + counts.0, acc.1 + counts.1, acc.2 + counts.2)
// 	})
// }

// fn calculate_shape_overlays_per_type(shape: &VectorManipulatorShape) -> (usize, usize, usize) {
// 	let (mut total_anchors, mut total_handles, mut total_anchor_handle_lines) = (0, 0, 0);

// 	for segment in &shape.segments {
// 		let (anchors, handles, anchor_handle_lines) = match segment {
// 			VectorManipulatorSegment::Line(_, _) => (1, 0, 0),
// 			VectorManipulatorSegment::Quad(_, _, _) => (1, 1, 1),
// 			VectorManipulatorSegment::Cubic(_, _, _, _) => (1, 2, 2),
// 		};
// 		total_anchors += anchors;
// 		total_handles += handles;
// 		total_anchor_handle_lines += anchor_handle_lines;
// 	}

// 	// A non-closed shape does not reuse the start and end point, so there is one extra
// 	if !shape.closed {
// 		total_anchors += 1;
// 	}

// 	(total_anchors, total_handles, total_anchor_handle_lines)
// }
