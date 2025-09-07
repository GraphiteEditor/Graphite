use graph_craft::document::{NodeInput, NodeNetwork};
use graphene_std::{node_graph_overlay::types::NodeGraphOverlayData, uuid::NodeId};
use interpreted_executor::ui_runtime::CompilationRequest;

use crate::{
	dispatcher::{Dispatcher, EditorOutput},
	messages::side_effects::SideEffectMessage,
};

impl Dispatcher {
	pub fn handle_side_effect(&mut self, message: SideEffectMessage) -> Vec<EditorOutput> {
		let mut responses = Vec::new();
		match message {
			SideEffectMessage::RenderNodeGraph => {
				if let Some(node_graph_overlay_network) = self.generate_node_graph_overlay_network() {
					let compilation_request = CompilationRequest { network: node_graph_overlay_network };
					responses.push(EditorOutput::RequestNativeNodeGraphRender { compilation_request });
				}
			}
			SideEffectMessage::RequestDeferredMessage { message, timeout } => {
				responses.push(EditorOutput::RequestDeferredMessage { message, timeout });
			}
			_ => todo!(),
		};
		responses
	}

	pub fn generate_node_graph_overlay_network(&mut self) -> Option<NodeNetwork> {
		let Some(active_document) = self.message_handlers.portfolio_message_handler.active_document_mut() else {
			return None;
		};
		let breadcrumb_network_path = &active_document.breadcrumb_network_path;
		let nodes_to_render = active_document.network_interface.collect_nodes(
			&active_document.node_graph_handler.node_graph_errors,
			self.message_handlers.preferences_message_handler.graph_wire_style,
			breadcrumb_network_path,
		);
		let previewed_node = active_document.network_interface.previewed_node(breadcrumb_network_path);
		let node_graph_render_data = NodeGraphOverlayData {
			nodes_to_render,
			open: active_document.graph_view_overlay_open,
			in_selected_network: &active_document.selection_network_path == breadcrumb_network_path,
			previewed_node,
			thumbnails: active_document.node_graph_handler.thumbnails.clone(),
		};
		let opacity = active_document.graph_fade_artwork_percentage;
		let node_graph_overlay_node = crate::messages::portfolio::document::node_graph::generate_node_graph_overlay::generate_node_graph_overlay(node_graph_render_data, opacity);
		Some(NodeNetwork {
			exports: vec![NodeInput::node(NodeId(0), 0)],
			nodes: vec![(NodeId(0), node_graph_overlay_node)].into_iter().collect(),
			..Default::default()
		})
	}
}
