use crate::messages::prelude::*;
use graph_craft::document::NodeId;
use std::collections::{HashMap, HashSet};

/// The layers the frontend shows in view, as rows of the Layers panel and as nodes in the node graph, which are the only ones it's sent the thumbnails of.
/// The latest thumbnail of every layer waits here for whenever the layer comes into view.
#[derive(Debug, Clone, Default)]
pub struct LayersInView {
	/// The layers with a row in view in the Layers panel, which is also sent their details.
	layers_panel: HashSet<NodeId>,
	/// The nodes in view in the node graph while it's open.
	node_graph: HashSet<NodeId>,
	/// The message with the latest thumbnail of each layer, as rendered by the graph runtime.
	thumbnails: HashMap<NodeId, FrontendMessage>,
	/// The layers whose latest thumbnail the frontend has.
	thumbnails_sent: HashSet<NodeId>,
}

impl LayersInView {
	pub fn layers_panel(&self) -> &HashSet<NodeId> {
		&self.layers_panel
	}

	/// The layers in view in either the Layers panel or the node graph.
	pub fn all_in_view(&self) -> HashSet<NodeId> {
		self.layers_panel.union(&self.node_graph).copied().collect()
	}

	/// Sets the layers with a row in view in the Layers panel, sending the thumbnails it lacks for them.
	pub fn set_layers_panel(&mut self, layers: HashSet<NodeId>, responses: &mut VecDeque<Message>) {
		self.layers_panel = layers;
		self.send_thumbnails_in_view(responses);
		responses.add(PortfolioMessage::SubmitThumbnailsInView);
	}

	/// Sets the nodes in view in the node graph, sending the thumbnails it lacks for those that are layers.
	pub fn set_node_graph(&mut self, nodes: HashSet<NodeId>, responses: &mut VecDeque<Message>) {
		self.node_graph = nodes;
		self.send_thumbnails_in_view(responses);
		responses.add(PortfolioMessage::SubmitThumbnailsInView);
	}

	pub fn thumbnail_layer(message: &FrontendMessage) -> Option<NodeId> {
		match message {
			FrontendMessage::UpdateNodeThumbnail { id, .. } | FrontendMessage::UpdateNodeThumbnailCanvases { id, .. } | FrontendMessage::UpdateNodeThumbnailPixels { id, .. } => Some(*id),
			_ => None,
		}
	}

	/// Keeps the message with a layer's newly rendered thumbnail, sending it to the frontend only if the layer is in view.
	pub fn receive_thumbnail(&mut self, layer: NodeId, thumbnail: FrontendMessage, responses: &mut VecDeque<Message>) {
		if self.in_view(&layer) {
			responses.add(thumbnail.clone());
			self.thumbnails_sent.insert(layer);
		} else {
			// Out of view, the frontend's copy is now stale until the layer comes back into view
			self.thumbnails_sent.remove(&layer);
		}

		self.thumbnails.insert(layer, thumbnail);
	}

	/// Forgets the thumbnails the graph runtime dropped, which it renders anew for any layer that returns, and has the frontend drop its copies too.
	pub fn forget_thumbnails(&mut self, layers: &[NodeId], responses: &mut VecDeque<Message>) {
		// Only this document's, since the frontend keeps a background document's copies for switching back
		let ids = layers.iter().copied().filter(|layer| self.thumbnails.contains_key(layer)).collect::<Vec<_>>();
		self.forget_thumbnails_in_background(&ids);

		if !ids.is_empty() {
			responses.add(FrontendMessage::ClearNodeThumbnails { ids });
		}
	}

	/// Forgets the thumbnails the graph runtime dropped while this document is in the background, leaving the frontend its copies for when it's active again.
	pub fn forget_thumbnails_in_background(&mut self, layers: &[NodeId]) {
		for layer in layers {
			self.thumbnails.remove(layer);
			self.thumbnails_sent.remove(layer);
		}
	}

	fn in_view(&self, layer: &NodeId) -> bool {
		self.layers_panel.contains(layer) || self.node_graph.contains(layer)
	}

	// O(layers in view)
	fn send_thumbnails_in_view(&mut self, responses: &mut VecDeque<Message>) {
		for &layer in self.layers_panel.iter().chain(&self.node_graph) {
			if self.thumbnails_sent.contains(&layer) {
				continue;
			}
			let Some(thumbnail) = self.thumbnails.get(&layer) else { continue };

			responses.add(thumbnail.clone());
			self.thumbnails_sent.insert(layer);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn svg_thumbnail(layer: NodeId, svg: &str) -> FrontendMessage {
		FrontendMessage::UpdateNodeThumbnail { id: layer, value: svg.to_string() }
	}

	fn thumbnails_sent(responses: VecDeque<Message>) -> Vec<(NodeId, String)> {
		responses
			.into_iter()
			.filter_map(|message| match message {
				Message::Frontend(FrontendMessage::UpdateNodeThumbnail { id, value }) => Some((id, value)),
				_ => None,
			})
			.collect()
	}

	#[test]
	fn a_thumbnail_waits_until_its_layer_comes_into_view() {
		let (layer, other) = (NodeId(1), NodeId(2));
		let mut layers_in_view = LayersInView::default();

		let mut responses = VecDeque::new();
		layers_in_view.receive_thumbnail(layer, svg_thumbnail(layer, "first"), &mut responses);
		layers_in_view.receive_thumbnail(layer, svg_thumbnail(layer, "latest"), &mut responses);
		assert!(thumbnails_sent(responses).is_empty(), "out of view, a thumbnail isn't sent");

		let mut responses = VecDeque::new();
		layers_in_view.set_layers_panel(HashSet::from([layer, other]), &mut responses);
		assert_eq!(thumbnails_sent(responses), vec![(layer, "latest".to_string())], "coming into view, only the latest thumbnail is sent");

		let mut responses = VecDeque::new();
		layers_in_view.set_node_graph(HashSet::from([layer]), &mut responses);
		assert!(thumbnails_sent(responses).is_empty(), "a thumbnail the frontend already has isn't sent again");
	}

	#[test]
	fn a_thumbnail_changed_out_of_view_is_sent_once_back_in_view() {
		let layer = NodeId(1);
		let mut layers_in_view = LayersInView::default();

		let mut responses = VecDeque::new();
		layers_in_view.set_layers_panel(HashSet::from([layer]), &mut responses);
		layers_in_view.receive_thumbnail(layer, svg_thumbnail(layer, "in view"), &mut responses);
		assert_eq!(thumbnails_sent(responses), vec![(layer, "in view".to_string())], "in view, a thumbnail is sent as it's received");

		let mut responses = VecDeque::new();
		layers_in_view.set_layers_panel(HashSet::new(), &mut responses);
		layers_in_view.receive_thumbnail(layer, svg_thumbnail(layer, "changed"), &mut responses);
		layers_in_view.set_layers_panel(HashSet::from([layer]), &mut responses);
		assert_eq!(thumbnails_sent(responses), vec![(layer, "changed".to_string())]);
	}

	#[test]
	fn a_thumbnail_the_runtime_dropped_is_forgotten_until_rendered_again() {
		let (layer, other_documents_layer) = (NodeId(1), NodeId(2));
		let mut layers_in_view = LayersInView::default();

		let mut responses = VecDeque::new();
		layers_in_view.set_layers_panel(HashSet::from([layer]), &mut responses);
		layers_in_view.receive_thumbnail(layer, svg_thumbnail(layer, "before"), &mut responses);

		let mut responses = VecDeque::new();
		layers_in_view.forget_thumbnails(&[layer, other_documents_layer], &mut responses);
		let cleared = responses.into_iter().find_map(|message| match message {
			Message::Frontend(FrontendMessage::ClearNodeThumbnails { ids }) => Some(ids),
			_ => None,
		});
		assert_eq!(cleared, Some(vec![layer]), "the frontend drops only this document's copies");

		let mut responses = VecDeque::new();
		layers_in_view.set_layers_panel(HashSet::new(), &mut responses);
		layers_in_view.set_layers_panel(HashSet::from([layer]), &mut responses);
		assert!(thumbnails_sent(responses).is_empty(), "a forgotten thumbnail isn't sent again");

		let mut responses = VecDeque::new();
		layers_in_view.receive_thumbnail(layer, svg_thumbnail(layer, "after"), &mut responses);
		assert_eq!(thumbnails_sent(responses), vec![(layer, "after".to_string())], "rendered anew, it's sent as before");
	}
}
