use crate::consts::DEFAULT_STROKE_WIDTH;
use crate::messages::clipboard::utility_types::{ClipboardContent, ClipboardContentRaw, ClipboardItem, ClipboardLayer, ClipboardResource, ResourceData};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_network_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface;
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::utility_types::ToolType;
use graph_craft::application_io::resource::{DataSource, ResourceHash};
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::raster::Image;
use graphene_std::subpath::BezierHandles;
use graphene_std::vector::misc::HandleId;
use graphene_std::vector::{PointId, SegmentId, VectorModificationType};
use graphite_proc_macros::{ExtractField, message_handler_data};
use std::sync::Arc;

const CLIPBOARD_PREFIX: &str = "graphite: ";

#[derive(ExtractField)]
pub struct ClipboardMessageContext<'a> {
	pub portfolio: &'a mut PortfolioMessageHandler,
	pub current_tool: &'a ToolType,
	pub resource_storage: &'a ResourceStorageMessageHandler,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct ClipboardMessageHandler {}

#[message_handler_data]
impl MessageHandler<ClipboardMessage, ClipboardMessageContext<'_>> for ClipboardMessageHandler {
	fn process_message(&mut self, message: ClipboardMessage, responses: &mut std::collections::VecDeque<Message>, context: ClipboardMessageContext) {
		let ClipboardMessageContext {
			portfolio,
			current_tool,
			resource_storage,
		} = context;

		match message {
			ClipboardMessage::Cut => responses.add(FrontendMessage::TriggerSelectionRead { cut: true }),
			ClipboardMessage::Copy => responses.add(FrontendMessage::TriggerSelectionRead { cut: false }),
			ClipboardMessage::Paste => responses.add(FrontendMessage::TriggerClipboardRead),
			ClipboardMessage::ReadClipboard { content } => match content {
				ClipboardContentRaw::Text(text) => {
					if let Some(graphite) = text.strip_prefix(CLIPBOARD_PREFIX) {
						responses.add(ClipboardMessage::PasteItems { data: graphite.to_string() });
					} else {
						responses.add(FrontendMessage::TriggerSelectionWrite { content: text });
					}
				}
				ClipboardContentRaw::Svg(svg) => {
					responses.add(PortfolioMessage::InsertSvg {
						svg,
						name: None,
						mouse: None,
						parent_and_insert_index: None,
					});
				}
				ClipboardContentRaw::Image { data, width, height } => {
					responses.add(PortfolioMessage::InsertImage {
						image: Image::from_image_data(&data, width, height),
						name: None,
						mouse: None,
						parent_and_insert_index: None,
					});
				}
			},
			ClipboardMessage::ReadSelection { content, cut } => {
				if let Some(text) = content {
					responses.add(ClipboardMessage::Write {
						content: ClipboardContent::Text(text),
					});
				} else if cut {
					responses.add(ClipboardMessage::CutLayers);
				} else {
					responses.add(ClipboardMessage::CopyLayers);
				}
			}
			ClipboardMessage::Write { content } => {
				let text = match content {
					ClipboardContent::Svg(_) => {
						log::error!("SVG copying is not yet supported");
						return;
					}
					ClipboardContent::Image { .. } => {
						log::error!("Image copying is not yet supported");
						return;
					}
					ClipboardContent::Graphite(graphite) => format!("{CLIPBOARD_PREFIX}{graphite}"),
					ClipboardContent::Text(text) => text,
				};
				responses.add(FrontendMessage::TriggerClipboardWrite { content: text });
			}

			ClipboardMessage::CopyLayers => {
				if current_tool == &ToolType::Path {
					responses.add(PathToolMessage::Copy);
					return;
				}

				let Some(active_document) = portfolio.active_document_id.and_then(|id| portfolio.documents.get_mut(&id)) else {
					return;
				};

				if active_document.graph_view_overlay_open() {
					responses.add(NodeGraphMessage::Copy);
					return;
				}

				let mut buffer = Vec::new();

				let mut ordered_last_elements = active_document.network_interface.shallowest_unique_layers(&[]).collect::<Vec<_>>();
				ordered_last_elements.sort_by_key(|layer| {
					let Some(parent) = layer.parent(active_document.metadata()) else { return usize::MAX };
					DocumentMessageHandler::get_calculated_insert_index(active_document.metadata(), &SelectedNodes(vec![layer.to_node()]), parent)
				});

				for layer in ordered_last_elements.into_iter() {
					let layer_node_id = layer.to_node();

					let mut copy_ids = HashMap::new();
					copy_ids.insert(layer_node_id, NodeId(0));

					active_document
						.network_interface
						.upstream_flow_back_from_nodes(vec![layer_node_id], &[], network_interface::FlowType::LayerChildrenUpstreamFlow)
						.enumerate()
						.for_each(|(index, node_id)| {
							copy_ids.insert(node_id, NodeId((index + 1) as u64));
						});

					buffer.push(ClipboardLayer {
						nodes: active_document.network_interface.copy_nodes(&copy_ids, &[]).collect(),
						selected: active_document.network_interface.selected_nodes().selected_layers_contains(layer, active_document.metadata()),
						visible: active_document.network_interface.selected_nodes().layer_visible(layer, &active_document.network_interface),
						locked: active_document.network_interface.selected_nodes().layer_locked(layer, &active_document.network_interface),
						collapsed: false,
					});
				}

				responses.add(ClipboardMessage::WriteItems {
					items: buffer.into_iter().map(ClipboardItem::Layer).collect(),
				});
			}
			ClipboardMessage::CutLayers => {
				if current_tool == &ToolType::Path {
					responses.add(PathToolMessage::Cut);
					return;
				}

				if let Some(active_document) = portfolio.active_document()
					&& active_document.graph_view_overlay_open()
				{
					responses.add(NodeGraphMessage::Cut);
					return;
				}

				responses.add(ClipboardMessage::CopyLayers);
				responses.add(DocumentMessage::DeleteSelectedLayers);
			}
			ClipboardMessage::WriteItems { items } => {
				let has_content = items.iter().any(|item| match item {
					ClipboardItem::Layer(entry) => !entry.nodes.is_empty(),
					ClipboardItem::Nodes(nodes) => !nodes.is_empty(),
					ClipboardItem::Vector(vector) => !vector.is_empty(),
					ClipboardItem::Resource(_) => true,
				});
				if !has_content {
					return;
				}

				let mut resource_ids = HashSet::new();
				for item in &items {
					match item {
						ClipboardItem::Layer(entry) => entry
							.nodes
							.iter()
							.for_each(|(_, template)| network_interface::collect_node_resources(&template.document_node, &mut resource_ids)),
						ClipboardItem::Nodes(nodes) => nodes
							.iter()
							.for_each(|(_, template)| network_interface::collect_node_resources(&template.document_node, &mut resource_ids)),
						ClipboardItem::Vector(_) | ClipboardItem::Resource(_) => {}
					}
				}

				// Snapshot each resource's registry entry; embedded ones also need their bytes
				let mut resources = Vec::new();
				let mut bytes_to_load = Vec::new();
				if let Some(document) = portfolio.active_document() {
					for id in resource_ids {
						let Some(info) = document.resources.registry.info(&id) else { continue };
						if let Some(hash) = info.hash
							&& info.sources.contains(&DataSource::Embedded)
						{
							bytes_to_load.push((resources.len(), *hash));
						}
						resources.push(ClipboardResource {
							id,
							hash: info.hash.copied(),
							sources: info.sources.to_vec(),
							data: None,
						});
					}
				}

				if bytes_to_load.is_empty() {
					let mut items = items;
					items.extend(resources.into_iter().map(ClipboardItem::Resource));
					if let Some(content) = serialize_clipboard(&items) {
						responses.add(ClipboardMessage::Write { content });
					}
				} else {
					// Load the embedded bytes from the resource storage, then write
					let load_handle = resource_storage.resources();
					responses.add(async move {
						let mut resources = resources;
						for (index, hash) in bytes_to_load {
							if let Some(resource) = load_handle.load(hash).await {
								resources[index].data = Some(ResourceData(resource.as_ref().to_vec()));
							}
						}
						let mut items = items;
						items.extend(resources.into_iter().map(ClipboardItem::Resource));
						match serialize_clipboard(&items) {
							Some(content) => ClipboardMessage::Write { content }.into(),
							None => Message::NoOp,
						}
					});
				}
			}
			ClipboardMessage::PasteItems { data } => {
				let items = match serde_json::from_str::<Vec<ClipboardItem>>(&data) {
					Ok(items) => items,
					Err(error) => {
						log::error!("Failed to deserialize clipboard payload: {error}");
						return;
					}
				};

				let mut layers = Vec::new();
				let mut node_groups = Vec::new();
				let mut vectors = Vec::new();
				let mut resources = Vec::new();
				for item in items {
					match item {
						ClipboardItem::Layer(entry) => layers.push(entry),
						ClipboardItem::Nodes(nodes) => node_groups.push(nodes),
						ClipboardItem::Vector(vector) => vectors.push(vector),
						ClipboardItem::Resource(resource) => resources.push(resource),
					}
				}

				// Re-register the carried resources and store their bytes
				let mut needs_resolve = false;
				if !resources.is_empty()
					&& let Some(document) = portfolio.active_document_mut()
				{
					for resource in resources {
						if document.resources.registry.contains(&resource.id) {
							continue;
						}
						for source in resource.sources {
							document.resources.registry.push_source_back(&resource.id, source);
						}
						match resource.data {
							Some(data) => match resource.hash {
								Some(hash) if ResourceHash::from(data.0.as_slice()) == hash => {
									document.resources.registry.resolve(&resource.id, hash);
									responses.add(ResourceStorageMessage::Store { data: Arc::from(data.0) });
								}
								_ => warn!("Discarding pasted resource {:?}: embedded bytes do not match its advertised hash", resource.id),
							},
							None => needs_resolve = true,
						}
					}
				}

				// Paste the content through the existing per-type handlers
				if !layers.is_empty() {
					responses.add(ClipboardMessage::PasteLayers { entries: layers });
				}
				for nodes in node_groups {
					responses.add(NodeGraphMessage::InsertNodes { nodes });
				}
				for paths in vectors {
					responses.add(ClipboardMessage::PasteVectors { paths });
				}

				// Re-resolve resources that carried no bytes (URL- or font-backed)
				if needs_resolve && let Some(document_id) = portfolio.active_document_id {
					responses.add(PortfolioMessage::ResolveDocumentResources { document_id });
				}
			}
			ClipboardMessage::PasteLayers { entries } => {
				if let Some(document) = portfolio.active_document() {
					let parent = document.new_layer_parent(false);
					let mut all_new_ids = Vec::new();
					let mut layers = Vec::new();

					let mut added_nodes = false;

					for entry in entries.into_iter().rev() {
						let new_ids: HashMap<_, _> = entry.nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect();
						let Some(&root_id) = new_ids.get(&NodeId(0)) else {
							warn!("Skipping pasted layer missing its root node");
							continue;
						};
						let layer = LayerNodeIdentifier::new_unchecked(root_id);

						if !added_nodes {
							responses.add(DocumentMessage::DeselectAllLayers);
							responses.add(DocumentMessage::AddTransaction);
							added_nodes = true;
						}

						all_new_ids.extend(new_ids.values().cloned());

						responses.add(NodeGraphMessage::AddNodes { nodes: entry.nodes, new_ids });
						responses.add(NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index: 0 });
						layers.push(layer);
					}

					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: all_new_ids });
					responses.add(DeferMessage::AfterGraphRun {
						messages: vec![PortfolioMessage::CenterLayers { layers }.into()],
					});
				}
			}
			ClipboardMessage::PasteVectors { paths } => {
				// If using Path tool then send the operation to Path tool
				// TODO: Consider if this is accualy the correct place to put this logic
				// TODO: Consider making paste in general go through the current tool, so that the tool can decide what to do
				if *current_tool == ToolType::Path {
					responses.add(PathToolMessage::Paste { paths });
					return;
				}

				// If not using Path tool, create new layers and add paths into those
				if let Some(document) = portfolio.active_document() {
					let mut layers = Vec::new();

					for (_, new_vector, transform) in paths {
						let Some(node_type) = resolve_network_node_type("Path") else {
							error!("Path node does not exist");
							continue;
						};
						let nodes = vec![(NodeId(0), node_type.default_node_template())];

						let parent = document.new_layer_parent(false);

						let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);
						layers.push(layer);

						// Adding the transform back into the layer
						responses.add(GraphOperationMessage::TransformSet {
							layer,
							transform,
							transform_in: TransformIn::Local,
							skip_rerender: false,
						});

						// Add default fill and stroke to the layer
						let fill = graphene_std::vector::style::Fill::solid(Color::WHITE);
						responses.add(GraphOperationMessage::FillSet { layer, fill });

						let stroke = graphene_std::vector::style::Stroke::new(Some(Color::BLACK), DEFAULT_STROKE_WIDTH);
						responses.add(GraphOperationMessage::StrokeSet { layer, stroke });

						// Create new point ids and add those into the existing Vector path
						let mut points_map = HashMap::new();
						for (point, position) in new_vector.point_domain.iter() {
							let new_point_id = PointId::generate();
							points_map.insert(point, new_point_id);
							let modification_type = VectorModificationType::InsertPoint { id: new_point_id, position };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}

						// Create new segment ids and add the segments into the existing Vector path
						let mut segments_map = HashMap::new();
						for (segment_id, bezier, start, end) in new_vector.segment_bezier_iter() {
							let (Some(&start_point), Some(&end_point)) = (points_map.get(&start), points_map.get(&end)) else {
								warn!("Skipping pasted vector segment with an unknown endpoint");
								continue;
							};

							let new_segment_id = SegmentId::generate();
							segments_map.insert(segment_id, new_segment_id);

							let handles = match bezier.handles {
								BezierHandles::Linear => [None, None],
								BezierHandles::Quadratic { handle } => [Some(handle - bezier.start), None],
								BezierHandles::Cubic { handle_start, handle_end } => [Some(handle_start - bezier.start), Some(handle_end - bezier.end)],
							};

							let points = [start_point, end_point];
							let modification_type = VectorModificationType::InsertSegment { id: new_segment_id, points, handles };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}

						// Set G1 continuity
						for handles in new_vector.colinear_manipulators {
							let to_new_handle = |handle: HandleId| -> Option<HandleId> {
								Some(HandleId {
									ty: handle.ty,
									segment: *segments_map.get(&handle.segment)?,
								})
							};
							let (Some(first), Some(second)) = (to_new_handle(handles[0]), to_new_handle(handles[1])) else {
								continue;
							};
							let modification_type = VectorModificationType::SetG1Continuous {
								handles: [first, second],
								enabled: true,
							};
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}
					}

					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(Message::Defer(DeferMessage::AfterGraphRun {
						messages: vec![PortfolioMessage::CenterLayers { layers }.into()],
					}));
				}
			}
		}
	}
	advertise_actions!(ClipboardMessageDiscriminant;
		Cut,
		Copy,
		Paste,
	);
}

/// Serialize the clipboard items, logging on failure.
fn serialize_clipboard(items: &[ClipboardItem]) -> Option<ClipboardContent> {
	match serde_json::to_string(items) {
		Ok(data) => Some(ClipboardContent::Graphite(data)),
		Err(error) => {
			log::error!("Failed to serialize clipboard payload: {error}");
			None
		}
	}
}

#[cfg(test)]
mod test {
	use crate::messages::clipboard::utility_types::{ClipboardItem, ClipboardResource, ResourceData};
	use crate::test_utils::test_prelude::*;
	use graph_craft::application_io::resource::{DataSource, ResourceHash, ResourceId};

	/// Create an editor with three layers
	/// 1. A red rectangle
	/// 2. A blue shape
	/// 3. A green ellipse
	async fn create_editor_with_three_layers() -> EditorTestUtils {
		let mut editor = EditorTestUtils::create();

		editor.new_document().await;

		editor.select_primary_color(Color::RED).await;
		editor.draw_rect(100., 200., 300., 400.).await;

		editor.select_primary_color(Color::BLUE).await;
		editor.draw_polygon(10., 1200., 1300., 400.).await;

		editor.select_primary_color(Color::GREEN).await;
		editor.draw_ellipse(104., 1200., 1300., 400.).await;

		editor
	}

	/// Copies the layer selection and returns the written clipboard payload.
	async fn copy_layers_to_clipboard(editor: &mut EditorTestUtils) -> String {
		editor
			.handle_message(ClipboardMessage::CopyLayers)
			.await
			.into_iter()
			.find_map(|message| match message {
				FrontendMessage::TriggerClipboardWrite { content } => Some(content),
				_ => None,
			})
			.expect("copying layers should write a payload to the clipboard")
	}

	/// Pastes a clipboard string as if read from the system clipboard.
	async fn paste_from_clipboard(editor: &mut EditorTestUtils, clipboard: &str) {
		editor
			.handle_message(ClipboardMessage::ReadClipboard {
				content: ClipboardContentRaw::Text(clipboard.to_string()),
			})
			.await;
	}

	/// - create rect, shape and ellipse
	/// - copy
	/// - paste
	/// - assert that ellipse was copied
	#[tokio::test]
	async fn copy_paste_single_layer() {
		let mut editor = create_editor_with_three_layers().await;

		let layers_before_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();
		let clipboard = copy_layers_to_clipboard(&mut editor).await;
		paste_from_clipboard(&mut editor, &clipboard).await;

		let layers_after_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i + 1]);
		}
	}

	#[cfg_attr(miri, ignore)]
	/// - create rect, shape and ellipse
	/// - select shape
	/// - copy
	/// - paste
	/// - assert that shape was copied
	#[tokio::test]
	async fn copy_paste_single_layer_from_middle() {
		let mut editor = create_editor_with_three_layers().await;

		let layers_before_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();
		let shape_id = editor.active_document().metadata().all_layers().nth(1).unwrap();

		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![shape_id.to_node()] }).await;
		let clipboard = copy_layers_to_clipboard(&mut editor).await;
		paste_from_clipboard(&mut editor, &clipboard).await;

		let layers_after_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i + 1]);
		}
	}

	#[cfg_attr(miri, ignore)]
	/// - create rect, shape and ellipse
	/// - select ellipse and rect
	/// - copy
	/// - delete
	/// - create another rect
	/// - paste
	/// - paste
	#[tokio::test]
	async fn copy_paste_deleted_layers() {
		let mut editor = create_editor_with_three_layers().await;
		assert_eq!(editor.active_document().metadata().all_layers().count(), 3);

		let layers_before_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();
		let rect_id = layers_before_copy[0];
		let shape_id = layers_before_copy[1];
		let ellipse_id = layers_before_copy[2];

		editor
			.handle_message(NodeGraphMessage::SelectedNodesSet {
				nodes: vec![rect_id.to_node(), ellipse_id.to_node()],
			})
			.await;
		let clipboard = copy_layers_to_clipboard(&mut editor).await;
		editor.handle_message(NodeGraphMessage::DeleteSelectedNodes { delete_children: true }).await;
		editor.draw_rect(0., 800., 12., 200.).await;
		paste_from_clipboard(&mut editor, &clipboard).await;
		paste_from_clipboard(&mut editor, &clipboard).await;

		let layers_after_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 6);

		assert_eq!(layers_after_copy[5], shape_id);
	}

	/// A pasted `graphite:` payload re-registers the resources it carries into the active document.
	#[tokio::test]
	async fn paste_carries_embedded_resource() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let id = ResourceId::new();
		assert!(!editor.active_document().resources.registry.contains(&id));

		// Carry the resource and its bytes through the clipboard
		let bytes = b"a pretend png".to_vec();
		let hash = ResourceHash::from(bytes.as_slice());
		let items = vec![ClipboardItem::Resource(ClipboardResource {
			id,
			hash: Some(hash),
			sources: vec![DataSource::Embedded],
			data: Some(ResourceData(bytes)),
		})];
		let payload = format!("graphite: {}", serde_json::to_string(&items).unwrap());

		editor
			.handle_message(ClipboardMessage::ReadClipboard {
				content: ClipboardContentRaw::Text(payload),
			})
			.await;

		let registry = &editor.active_document().resources.registry;
		assert!(registry.contains(&id), "the carried resource should be registered after paste");
		assert_eq!(registry.hash(&id), Some(hash), "the carried resource should resolve to the carried bytes' hash");
	}
}
