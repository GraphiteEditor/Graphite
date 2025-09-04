use crate::dispatcher::Dispatcher;
use crate::messages::portfolio::document::node_graph::generate_node_graph_overlay::generate_node_graph_overlay;
use crate::messages::prelude::*;
use graph_craft::document::{NodeInput, NodeNetwork};
use graphene_std::node_graph_overlay::types::NodeGraphOverlayData;
pub use graphene_std::uuid::*;

// TODO: serialize with serde to save the current editor state
pub struct Editor {
	pub dispatcher: Dispatcher,
}

impl Editor {
	/// Construct the editor.
	/// Remember to provide a random seed with `editor::set_uuid_seed(seed)` before any editors can be used.
	pub fn new() -> Self {
		Self { dispatcher: Dispatcher::new() }
	}

	#[cfg(test)]
	pub(crate) fn new_local_executor() -> (Self, crate::node_graph_executor::NodeRuntime) {
		let (runtime, executor) = crate::node_graph_executor::NodeGraphExecutor::new_with_local_runtime();
		let dispatcher = Dispatcher::with_executor(executor);
		(Self { dispatcher }, runtime)
	}

	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Vec<FrontendMessage> {
		self.dispatcher.handle_message(message, true);

		std::mem::take(&mut self.dispatcher.responses)
	}

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) -> Result<(), String> {
		self.dispatcher.poll_node_graph_evaluation(responses)
	}

	pub fn generate_node_graph_overlay_network(&mut self) -> Option<NodeNetwork> {
		let Some(active_document) = self.dispatcher.message_handlers.portfolio_message_handler.active_document_mut() else {
			return None;
		};
		let breadcrumb_network_path = &active_document.breadcrumb_network_path;
		let nodes_to_render = active_document.network_interface.collect_nodes(
			&active_document.node_graph_handler.node_graph_errors,
			self.dispatcher.message_handlers.preferences_message_handler.graph_wire_style,
			breadcrumb_network_path,
		);
		let previewed_node = active_document.network_interface.previewed_node(breadcrumb_network_path);
		let node_graph_render_data = NodeGraphOverlayData {
			nodes_to_render,
			open: active_document.graph_view_overlay_open,
			in_selected_network: &active_document.selection_network_path == breadcrumb_network_path,
			previewed_node,
		};
		let opacity = active_document.graph_fade_artwork_percentage;
		let font_cache = self.dispatcher.message_handlers.portfolio_message_handler.persistent_data.font_cache.clone();
		let node_graph_overlay_node = generate_node_graph_overlay(node_graph_render_data, opacity, font_cache);
		Some(NodeNetwork {
			exports: vec![NodeInput::node(NodeId(0), 0)],
			nodes: vec![(NodeId(0), node_graph_overlay_node)].into_iter().collect(),
			..Default::default()
		})
	}
}

impl Default for Editor {
	fn default() -> Self {
		Self::new()
	}
}

pub const GRAPHITE_RELEASE_SERIES: &str = env!("GRAPHITE_RELEASE_SERIES");
pub const GRAPHITE_GIT_COMMIT_DATE: &str = env!("GRAPHITE_GIT_COMMIT_DATE");
pub const GRAPHITE_GIT_COMMIT_HASH: &str = env!("GRAPHITE_GIT_COMMIT_HASH");
pub const GRAPHITE_GIT_COMMIT_BRANCH: &str = env!("GRAPHITE_GIT_COMMIT_BRANCH");

pub fn commit_info_localized(localized_commit_date: &str) -> String {
	format!(
		"Release Series: {}\n\
		Branch: {}\n\
		Commit: {}\n\
		{}",
		GRAPHITE_RELEASE_SERIES,
		GRAPHITE_GIT_COMMIT_BRANCH,
		&GRAPHITE_GIT_COMMIT_HASH[..8],
		localized_commit_date
	)
}
