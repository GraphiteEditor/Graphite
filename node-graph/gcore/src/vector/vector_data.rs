use glam::DAffine2;

use super::style::PathStyle;
use crate::uuid::ManipulatorGroupId;

/// [VectorData] is passed between nodes.
/// It contains a list of subpaths (that may be open or closed), a transform and some style information.
pub struct VectorData {
	pub subpaths: Vec<bezier_rs::Subpath<ManipulatorGroupId>>,
	pub transform: DAffine2,
	pub style: PathStyle,
}
