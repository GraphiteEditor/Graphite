use bezier_rs::Subpath;
use glam::DAffine2;
use graphene_core::renderer::{ClickTarget, Quad};
use graphene_core::uuid::ManipulatorGroupId;

#[derive(Debug, Clone)]
pub struct NodeMetadata {
	/// Cache for all node click targets in node graph space. Ensure update_click_target is called when modifying a node property that changes its size. Currently this is alias, inputs, is_layer, and metadata
	pub node_click_target: ClickTarget,
	/// Cache for all node inputs. Should be automatically updated when update_click_target is called
	pub input_click_targets: Vec<ClickTarget>,
	/// Cache for all node outputs. Should be automatically updated when update_click_target is called
	pub output_click_targets: Vec<ClickTarget>,
	/// Cache for all visibility buttons. Should be automatically updated when update_click_target is called
	pub visibility_click_target: ClickTarget,
	/// Stores the width in grid cell units from the left edge of the thumbnail (+12px padding since thumbnail ends between grid spaces) to the end of the node
	pub layer_width: u32,
}

#[derive(Debug, Clone)]
pub struct NetworkMetadata {
	/// Cache for the bounding box around all nodes in node graph space.
	pub bounding_box_subpath: Option<Subpath<ManipulatorGroupId>>,
	/// Transform from node graph space to viewport space.
	pub node_graph_to_viewport: DAffine2,
}
