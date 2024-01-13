use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::fmt;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DocumentId(pub u64);

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, Hash)]
pub enum FlipAxis {
	X,
	Y,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, Hash, specta::Type)]
pub enum AlignAxis {
	X,
	Y,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, Hash, specta::Type)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DocumentMode {
	#[default]
	DesignMode,
	SelectMode,
	GuideMode,
}

impl fmt::Display for DocumentMode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			DocumentMode::DesignMode => write!(f, "Design Mode"),
			DocumentMode::SelectMode => write!(f, "Select Mode"),
			DocumentMode::GuideMode => write!(f, "Guide Mode"),
		}
	}
}

impl DocumentMode {
	pub fn icon_name(&self) -> String {
		match self {
			DocumentMode::DesignMode => "ViewportDesignMode".to_string(),
			DocumentMode::SelectMode => "ViewportSelectMode".to_string(),
			DocumentMode::GuideMode => "ViewportGuideMode".to_string(),
		}
	}
}

#[derive(Clone, Debug)]
/// SnappingState determines the current individual snapping states
pub struct SnappingState {
	pub snapping_enabled: bool,
	pub bounding_box_snapping: bool,
	pub node_snapping: bool,
}

impl Default for SnappingState {
	fn default() -> Self {
		Self {
			snapping_enabled: true,
			bounding_box_snapping: true,
			node_snapping: true,
		}
	}
}

// TODO: implement icons for SnappingOptions eventually
pub enum SnappingOptions {
	BoundingBoxes,
	Points,
}

impl fmt::Display for SnappingOptions {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			SnappingOptions::BoundingBoxes => write!(f, "Bounding Boxes"),
			SnappingOptions::Points => write!(f, "Points"),
		}
	}
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PTZ {
	pub pan: DVec2,
	pub tilt: f64,
	pub zoom: f64,
}

impl Default for PTZ {
	fn default() -> Self {
		Self { pan: DVec2::ZERO, tilt: 0., zoom: 1. }
	}
}
