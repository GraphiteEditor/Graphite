use crate::consts::DEFAULT_STROKE_WIDTH;
use crate::messages::portfolio::document::SelectionParts;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_network_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface;
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::messages::portfolio::ingest::utility_types::{ApplicationDataItem, ApplicationDataLayer, ApplicationDataResource, DataTransferItem, ResourceData, TransferData};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::utility_types::ToolType;
use graph_craft::application_io::resource::{DataSource, ResourceHash};
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::list::List;
use graphene_std::vector::misc::{BezierHandles, HandleId, point_to_dvec2, segment_to_handles};
use graphene_std::vector::{PointId, SegmentId, Vector, VectorModificationType};
use graphite_proc_macros::{ExtractField, message_handler_data};
use kurbo::ParamCurve;
use std::sync::Arc;

#[derive(ExtractField)]
pub struct ClipboardMessageContext<'a> {
	pub portfolio: &'a mut PortfolioMessageHandler,
	pub current_tool: &'a ToolType,
	pub resource_storage: &'a ResourceStorageMessageHandler,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct ClipboardMessageHandler {
	pending_copy: Option<PendingCopy>,
	copy_id: u64,
}

#[derive(Debug, Clone)]
enum PendingCopy {
	Rendering {
		id: u64,
		items: Vec<ApplicationDataItem>,
		loads: Vec<(usize, ResourceHash)>,
	},
	Loading {
		id: u64,
	},
}

impl PendingCopy {
	fn id(&self) -> u64 {
		match self {
			Self::Rendering { id, .. } | Self::Loading { id } => *id,
		}
	}
}

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
			ClipboardMessage::ReadSelection { content, cut } => {
				if let Some(text) = content {
					responses.add(ClipboardMessage::Write {
						content: TransferData::Text(text).into(),
					});
				} else if cut {
					responses.add(ClipboardMessage::CutLayers);
				} else {
					responses.add(ClipboardMessage::CopyLayers);
				}
			}
			ClipboardMessage::Write { content } => {
				self.pending_copy = None;
				responses.add(FrontendMessage::TriggerClipboardWrite { content });
			}

			ClipboardMessage::CopyLayers => {
				let Some(active_document) = portfolio.active_document() else { return };

				// The Path tool copies the points it's editing in the viewport
				if current_tool == &ToolType::Path && !active_document.graph_view_overlay_open() {
					responses.add(PathToolMessage::Copy);
					return;
				}

				let SelectionParts { layers, chain_nodes, other_nodes } = active_document.partition_selection();
				let mut items = Vec::new();

				let mut ordered_last_elements = layers;
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

					// The layer panel keys collapse state on the top-down path of node IDs down to the layer
					let mut tree_path = layer
						.ancestors(active_document.metadata())
						.filter(|ancestor| *ancestor != LayerNodeIdentifier::ROOT_PARENT)
						.map(|ancestor| ancestor.to_node())
						.collect::<Vec<_>>();
					tree_path.reverse();

					items.push(ApplicationDataItem::Layer(ApplicationDataLayer {
						nodes: active_document.network_interface.copy_nodes(&copy_ids, &[]).collect(),
						visible: active_document.network_interface.selected_nodes().layer_visible(layer, &active_document.network_interface),
						locked: active_document.network_interface.selected_nodes().layer_locked(layer, &active_document.network_interface),
						collapsed: active_document.collapsed.0.contains(&tree_path),
					}));
				}

				// The dots on wires between the copied chain nodes go along with them, so pasting them loose keeps those wires
				let chain_nodes_with_dots = active_document.network_interface.selection_with_bridging_dots(chain_nodes, &[]);
				let other_nodes = other_nodes.into_iter().filter(|node_id| !chain_nodes_with_dots.contains(node_id)).collect::<Vec<_>>();

				let copies = chain_nodes_with_dots
					.iter()
					.filter_map(|node_id| active_document.network_interface.copy_chain_node(node_id, &[]))
					.collect::<Vec<_>>();
				if !copies.is_empty() {
					items.push(ApplicationDataItem::ChainNodes(copies));
				}

				// Other nodes are kept under their own IDs, so a paste can tell when it's over the same nodes
				if !other_nodes.is_empty() {
					let copy_ids = active_document.with_layer_content(&other_nodes).into_iter().map(|node_id| (node_id, node_id)).collect();
					let nodes = active_document.network_interface.copy_nodes(&copy_ids, active_document.selection_network_path()).collect();
					items.push(ApplicationDataItem::Nodes(nodes));
				}

				responses.add(ClipboardMessage::WriteItems { items });
			}
			ClipboardMessage::CutLayers => {
				let Some(active_document) = portfolio.active_document() else { return };

				if current_tool == &ToolType::Path && !active_document.graph_view_overlay_open() {
					responses.add(PathToolMessage::Cut);
					return;
				}

				let SelectionParts { layers, chain_nodes, other_nodes } = active_document.partition_selection();
				responses.add(ClipboardMessage::CopyLayers);

				// Layers and other nodes go along with what only they use, while chain nodes go alone so pasting them can wire back to what fed them
				let deleted_with_children = layers.iter().map(|layer| layer.to_node()).chain(other_nodes).collect::<Vec<_>>();
				if deleted_with_children.is_empty() && chain_nodes.is_empty() {
					return;
				}
				responses.add(DocumentMessage::AddTransaction);
				if !deleted_with_children.is_empty() {
					responses.add(NodeGraphMessage::DeleteNodes {
						node_ids: deleted_with_children,
						delete_children: true,
					});
				}
				if !chain_nodes.is_empty() {
					responses.add(NodeGraphMessage::DeleteNodes {
						node_ids: chain_nodes,
						delete_children: false,
					});
				}
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::SelectedNodesUpdated);
				responses.add(NodeGraphMessage::SendGraph);
			}
			ClipboardMessage::WriteItems { mut items } => {
				let has_content = items.iter().any(|item| match item {
					ApplicationDataItem::Layer(entry) => !entry.nodes.is_empty(),
					ApplicationDataItem::Nodes(nodes) => !nodes.is_empty(),
					ApplicationDataItem::ChainNodes(copies) => !copies.is_empty(),
					ApplicationDataItem::Vector(vector) => !vector.is_empty(),
					ApplicationDataItem::Resource(_) => true,
					ApplicationDataItem::Definition(_) => false,
				});
				if !has_content {
					return;
				}

				// The table entries the copied nodes name travel with them, so a paste into another document can add them
				let definitions = portfolio
					.active_document()
					.map(|document| document.network_interface.copied_definitions(items.iter().flat_map(item_templates)))
					.unwrap_or_default();

				let mut resource_ids = HashSet::new();
				for template in items.iter().flat_map(item_templates) {
					network_interface::collect_template_resources(template, &mut resource_ids);
				}
				for definition in &definitions {
					network_interface::visit_network_resources(&definition.network, &mut |id| {
						resource_ids.insert(id);
					});
				}
				items.extend(definitions.into_iter().map(|definition| ApplicationDataItem::Definition(Box::new(definition))));

				// Snapshot each resource's registry entry; embedded ones also need their bytes
				let mut loads = Vec::new();
				if let Some(document) = portfolio.active_document() {
					for id in resource_ids {
						let Some(info) = document.resources.registry.info(&id) else { continue };
						if let Some(hash) = info.hash
							&& info.sources.contains(&DataSource::Embedded)
						{
							loads.push((items.len(), *hash));
						}
						items.push(ApplicationDataItem::Resource(ApplicationDataResource {
							id,
							hash: info.hash.copied(),
							sources: info.sources.to_vec(),
							data: None,
						}));
					}
				}

				if items.iter().any(|item| matches!(item, ApplicationDataItem::Layer(_))) {
					let id = self.next_copy_id();
					self.pending_copy = Some(PendingCopy::Rendering { id, items, loads });
					responses.add(PortfolioMessage::SubmitClipboardSvg { id });
				} else {
					self.finish_copy(items, loads, None, resource_storage, responses);
				}
			}
			ClipboardMessage::Rendered { id, svg } => {
				let Some(PendingCopy::Rendering { items, loads, .. }) = self.pending_copy.take_if(|pending| pending.id() == id) else {
					return;
				};
				self.finish_copy(items, loads, svg, resource_storage, responses);
			}
			ClipboardMessage::Loaded { id, content } => {
				if self.pending_copy.as_ref().is_some_and(|pending| pending.id() == id) {
					responses.add(ClipboardMessage::Write { content });
				}
			}
			ClipboardMessage::PasteItems { items } => {
				let mut layers = Vec::new();
				let mut node_groups = Vec::new();
				let mut chain_node_groups = Vec::new();
				let mut vectors = Vec::new();
				let mut resources = Vec::new();
				let mut definitions = Vec::new();
				for item in items {
					// Pasted templates skip document migration, so stored types normalize here
					match item {
						ApplicationDataItem::Layer(mut entry) => {
							for (_, template) in &mut entry.nodes {
								template.normalize_stored_types();
							}
							layers.push(entry);
						}
						ApplicationDataItem::Nodes(mut nodes) => {
							for (_, template) in &mut nodes {
								template.normalize_stored_types();
							}
							node_groups.push(nodes);
						}
						ApplicationDataItem::ChainNodes(mut copies) => {
							for copy in &mut copies {
								copy.template.normalize_stored_types();
							}
							chain_node_groups.push(copies);
						}
						ApplicationDataItem::Vector(vector) => vectors.push(vector),
						ApplicationDataItem::Resource(resource) => resources.push(resource),
						ApplicationDataItem::Definition(definition) => definitions.push(*definition),
					}
				}

				// The carried table entries go in as the nodes naming them do, renamed where this document holds a different entry under the same ID
				if !definitions.is_empty()
					&& let Some(document) = portfolio.active_document_mut()
				{
					let Some(renamed) = document.network_interface.carry_definitions(definitions) else {
						log::error!("Discarding the paste: its subgraphs name one another in a cycle or name one it does not carry");
						return;
					};
					if !renamed.is_empty() {
						let templates = layers
							.iter_mut()
							.flat_map(|entry| entry.nodes.iter_mut().map(|(_, template)| template))
							.chain(node_groups.iter_mut().flatten().map(|(_, template)| template))
							.chain(chain_node_groups.iter_mut().flatten().map(|copy| &mut copy.template));
						for template in templates {
							template.rename_definitions(&renamed);
						}
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
					responses.add(DocumentMessage::PasteNodes { nodes });
				}
				for copies in chain_node_groups {
					responses.add(DocumentMessage::PasteChainNodes { copies });
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

					// Top-down node ID path of the destination parent, used to derive each pasted layer's collapse path
					let mut parent_tree_path = parent
						.ancestors(document.metadata())
						.filter(|ancestor| *ancestor != LayerNodeIdentifier::ROOT_PARENT)
						.map(|ancestor| ancestor.to_node())
						.collect::<Vec<_>>();
					parent_tree_path.reverse();

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

						responses.add(NodeGraphMessage::AddNodes { nodes: entry.nodes, new_ids });
						responses.add(NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index: 0 });

						// Restore the copied layer's visibility, lock, and collapse state
						if !entry.visible {
							responses.add(NodeGraphMessage::SetVisibility {
								node_id: root_id,
								network_path: Vec::new(),
								visible: false,
							});
						}
						if entry.locked {
							responses.add(NodeGraphMessage::SetLocked {
								node_id: root_id,
								network_path: Vec::new(),
								locked: true,
							});
						}
						if entry.collapsed {
							let mut tree_path = parent_tree_path.clone();
							tree_path.push(root_id);
							responses.add(DocumentMessage::ToggleLayerExpansion { tree_path, recursive: false });
						}

						layers.push(layer);
					}

					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(NodeGraphMessage::SelectedNodesSet {
						nodes: layers.iter().map(|layer| layer.to_node()).collect(),
					});
					responses.add(DeferMessage::AfterGraphRun {
						messages: vec![PortfolioMessage::CenterLayers { layers }.into()],
					});
				}
			}
			ClipboardMessage::PasteVectors { paths } => {
				// The Path tool pastes points into the path it's editing in the viewport
				// TODO: Consider if this is actually the correct place to put this logic
				// TODO: Consider making paste in general go through the current tool, so that the tool can decide what to do
				if *current_tool == ToolType::Path && portfolio.active_document().is_some_and(|document| !document.graph_view_overlay_open()) {
					responses.add(PathToolMessage::Paste { paths });
					return;
				}

				// Otherwise, create new layers and add paths into those
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
						responses.add(GraphOperationMessage::FillColorSet { layer, color: Some(Color::WHITE) });

						let color = Some(Color::BLACK);
						let stroke = graphene_std::vector::style::Stroke::new(DEFAULT_STROKE_WIDTH);
						responses.add(GraphOperationMessage::StrokeSet { layer, color, stroke });

						// Materialize the copied snapshot so its points and segments can replay with fresh IDs
						let (mut pasted_vector, mut pasted_paths) = (Vector::default(), List::new());
						new_vector.apply(&mut pasted_vector, &mut pasted_paths);

						// Create new point IDs and add those into the existing Vector path
						let mut points_map = HashMap::new();
						for index in pasted_vector.anchor_indices(&pasted_paths) {
							let (Some(point), Some(position)) = (pasted_vector.point_id_at(index as u32), pasted_vector.position(index)) else {
								continue;
							};

							let new_point_id = PointId::generate();
							points_map.insert(point, new_point_id);
							let modification_type = VectorModificationType::InsertPoint { id: new_point_id, position };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}

						// Create new segment IDs and add the segments into the existing Vector path
						let mut segments_map = HashMap::new();
						let path_beginnings = pasted_vector.path_beginnings(&pasted_paths);
						for (segment_id, segment, start, end) in pasted_vector.segment_iter(&pasted_paths) {
							let (Some(&start_point), Some(&end_point)) = (points_map.get(&start), points_map.get(&end)) else {
								warn!("Skipping pasted vector segment with an unknown endpoint");
								continue;
							};

							let new_segment_id = SegmentId::generate();
							segments_map.insert(segment_id, new_segment_id);

							let segment_start = point_to_dvec2(segment.start());
							let segment_end = point_to_dvec2(segment.end());
							let handles = match segment_to_handles(&segment) {
								BezierHandles::Linear => [None, None],
								BezierHandles::Quadratic { handle } => [Some(handle - segment_start), None],
								BezierHandles::Cubic { handle_start, handle_end } => [Some(handle_start - segment_start), Some(handle_end - segment_end)],
							};

							let points = [start_point, end_point];
							let modification_type = VectorModificationType::InsertSegment { id: new_segment_id, points, handles };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });

							// Every pasted path stays a path of its own, even where it sets out from the end of another
							if path_beginnings.contains(&segment_id) {
								let modification_type = VectorModificationType::SetPathBreak {
									segment: new_segment_id,
									breaks: true,
								};
								responses.add(GraphOperationMessage::Vector { layer, modification_type });
							}
						}

						// Set G1 continuity
						for handles in pasted_vector.colinear_pairs(&pasted_paths) {
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

impl ClipboardMessageHandler {
	fn next_copy_id(&mut self) -> u64 {
		self.copy_id += 1;
		self.copy_id
	}

	fn finish_copy(
		&mut self,
		mut items: Vec<ApplicationDataItem>,
		loads: Vec<(usize, ResourceHash)>,
		svg: Option<String>,
		resource_storage: &ResourceStorageMessageHandler,
		responses: &mut VecDeque<Message>,
	) {
		if loads.is_empty() {
			let content = [Some(TransferData::ApplicationData(items)), svg.map(TransferData::Svg)]
				.into_iter()
				.flatten()
				.map(DataTransferItem::from)
				.collect();
			responses.add(ClipboardMessage::Write { content });
			return;
		}

		let id = self.next_copy_id();
		self.pending_copy = Some(PendingCopy::Loading { id });

		let load_handle = resource_storage.resources();
		responses.add(async move {
			for (index, hash) in loads {
				if let ApplicationDataItem::Resource(resource) = &mut items[index]
					&& let Some(data) = load_handle.load(hash).await
				{
					resource.data = Some(ResourceData(data.as_ref().to_vec()));
				}
			}
			let content = [Some(TransferData::ApplicationData(items)), svg.map(TransferData::Svg)]
				.into_iter()
				.flatten()
				.map(DataTransferItem::from)
				.collect();
			Message::from(ClipboardMessage::Loaded { id, content })
		});
	}
}

/// The node templates a clipboard item carries.
fn item_templates(item: &ApplicationDataItem) -> Vec<&network_interface::NodeTemplate> {
	match item {
		ApplicationDataItem::Layer(entry) => entry.nodes.iter().map(|(_, template)| template).collect(),
		ApplicationDataItem::Nodes(nodes) => nodes.iter().map(|(_, template)| template).collect(),
		ApplicationDataItem::ChainNodes(copies) => copies.iter().map(|copy| &copy.template).collect(),
		ApplicationDataItem::Vector(_) | ApplicationDataItem::Resource(_) | ApplicationDataItem::Definition(_) => Vec::new(),
	}
}

#[cfg(test)]
mod test {
	use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
	use crate::messages::portfolio::ingest::utility_types::{ApplicationDataItem, ApplicationDataResource, DataType, RawData, ResourceData};
	use crate::test_utils::test_prelude::*;
	use graph_craft::application_io::resource::{DataSource, ResourceHash, ResourceId};
	use graph_craft::document::NodeInput;

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
	async fn copy_layers_to_clipboard(editor: &mut EditorTestUtils) -> DataTransfer {
		editor
			.copy_to_clipboard(ClipboardMessage::CopyLayers)
			.await
			.expect("copying layers should write a payload to the clipboard")
	}

	fn copied_svg(clipboard: &DataTransfer) -> Option<&str> {
		clipboard.0.iter().find_map(|item| match &item.data {
			TransferData::Svg(svg) => Some(svg.as_str()),
			_ => None,
		})
	}

	fn is_svg_document(svg: &str) -> bool {
		svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:graphite=\"https://graphite.art\"") && svg.ends_with("</svg>")
	}

	/// - create rect, shape and ellipse
	/// - copy
	#[cfg_attr(miri, ignore)]
	#[tokio::test]
	async fn copied_svg_paints_the_layers_from_the_bottom_up() {
		let mut editor = create_editor_with_three_layers().await;
		let layers = editor.active_document().metadata().all_layers().map(|layer| layer.to_node()).collect();
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: layers }).await;

		let clipboard = copy_layers_to_clipboard(&mut editor).await;
		let svg = copied_svg(&clipboard).expect("the copied layers should render an SVG");
		// The rectangle, polygon, and ellipse are told apart by where their outlines start
		let position = |path_start: &str| svg.find(path_start).unwrap_or_else(|| panic!("{path_start} should be in the SVG: {svg}"));
		let (rectangle, polygon, ellipse) = (position(r#"d="M-100,-100 "#), position(r#"d="M-645,-400 "#), position(r#"d="M0,-400 C"#));
		assert!(rectangle < polygon && polygon < ellipse, "The first drawn layer should be painted first: {svg}");
	}

	/// - paste
	/// - assert that ellipse was copied
	#[tokio::test]
	async fn copy_paste_single_layer() {
		let mut editor = create_editor_with_three_layers().await;

		let layers_before_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();
		let clipboard = copy_layers_to_clipboard(&mut editor).await;
		editor.paste_from_clipboard(clipboard.clone()).await;

		let layers_after_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i + 1]);
		}

		assert!(copied_svg(&clipboard).is_some_and(is_svg_document));
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
		editor.paste_from_clipboard(clipboard.clone()).await;

		let layers_after_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i + 1]);
		}

		assert!(copied_svg(&clipboard).is_some_and(is_svg_document));
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
		editor.paste_from_clipboard(clipboard.clone()).await;
		editor.paste_from_clipboard(clipboard.clone()).await;

		let layers_after_copy = editor.active_document().metadata().all_layers().collect::<Vec<_>>();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 6);

		assert_eq!(layers_after_copy[5], shape_id);
		assert!(copied_svg(&clipboard).is_some_and(is_svg_document));
	}

	#[cfg_attr(miri, ignore)]
	#[tokio::test]
	async fn cutting_an_embedded_image_carries_its_svg_and_bytes() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.create_raster_image(Image::new(2, 2, Color::RED), None).await;
		let layers = editor.active_document().metadata().all_layers().map(|layer| layer.to_node()).collect();
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: layers }).await;

		let clipboard = editor.copy_to_clipboard(ClipboardMessage::CutLayers).await.expect("cutting the image should write it to the clipboard");
		assert_eq!(editor.active_document().metadata().all_layers().count(), 0);
		assert!(copied_svg(&clipboard).is_some_and(is_svg_document), "the cut image should render an SVG: {clipboard:?}");

		let items = clipboard
			.0
			.iter()
			.find_map(|item| match &item.data {
				TransferData::ApplicationData(items) => Some(items),
				_ => None,
			})
			.expect("the cut image should be written as application data");
		assert!(items.iter().any(|item| matches!(item, ApplicationDataItem::Resource(ApplicationDataResource { data: Some(_), .. }))));
	}

	#[tokio::test]
	async fn copying_nodes_writes_only_graphite_data() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let rectangle = editor
			.create_node_by_name(DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER))
			.await;
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![rectangle] }).await;

		let clipboard = editor.copy_to_clipboard(NodeGraphMessage::Copy).await.expect("copying the node should write it to the clipboard");
		assert!(clipboard.0.iter().all(|item| matches!(item.data, TransferData::ApplicationData(_))), "{clipboard:?}");
	}

	#[cfg_attr(miri, ignore)]
	#[tokio::test]
	async fn copying_text_while_layers_render_keeps_the_text() {
		let mut editor = create_editor_with_three_layers().await;
		let layers = editor.active_document().metadata().all_layers().map(|layer| layer.to_node()).collect();
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: layers }).await;

		let mut written = editor.dispatch(ClipboardMessage::CopyLayers);
		written.extend(editor.dispatch(ClipboardMessage::ReadSelection {
			content: Some("text".to_string()),
			cut: false,
		}));
		editor.runtime.update();
		editor.runtime.run().await;
		written.extend(editor.dispatch(Message::Wake));

		let last = written
			.into_iter()
			.filter_map(|message| match message {
				FrontendMessage::TriggerClipboardWrite { content } => Some(content),
				_ => None,
			})
			.last();
		assert_eq!(last, Some(TransferData::Text("text".to_string()).into()));
	}

	const PASTED_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>"#;

	#[cfg_attr(miri, ignore)]
	#[tokio::test]
	async fn pasting_images_ingests_them() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer_count = |editor: &EditorTestUtils| editor.active_document().metadata().all_layers().count();

		editor.paste_from_clipboard(TransferData::Svg(PASTED_SVG.to_string()).into()).await;
		let after_svg = layer_count(&editor);
		assert!(after_svg > 0, "pasting an SVG should add it as a layer");

		let data = Image::new(2, 2, Color::RED).to_png();
		editor
			.paste_from_clipboard(
				TransferData::Raster {
					format: image::ImageFormat::Png,
					data,
				}
				.into(),
			)
			.await;
		assert!(layer_count(&editor) > after_svg, "pasting a PNG should add it as a layer");
	}

	#[cfg_attr(miri, ignore)]
	#[tokio::test]
	async fn pasting_unreadable_graphite_data_falls_back_to_its_svg() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let unreadable = DataTransferItem::from_raw(DataType::Text, RawData::Text("graphite: unreadable".to_string())).unwrap();
		let content = [unreadable, TransferData::Svg(PASTED_SVG.to_string()).into()].into_iter().collect();
		editor.paste_from_clipboard(content).await;
		assert!(editor.active_document().metadata().all_layers().count() > 0);
	}

	#[tokio::test]
	async fn pasting_text_inserts_it_at_the_caret() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let responses = editor.paste_from_clipboard(TransferData::Text("text".to_string()).into()).await;
		assert!(responses.contains(&FrontendMessage::TriggerSelectionWrite { content: "text".to_string() }));
	}

	#[tokio::test]
	async fn pasting_text_that_only_looks_like_application_data_inserts_it_at_the_caret() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let text = "graphite: not a payload".to_string();
		let content = DataTransferItem::from_raw(DataType::Text, RawData::Text(text.clone())).unwrap().into();
		let responses = editor.paste_from_clipboard(content).await;
		assert!(responses.contains(&FrontendMessage::TriggerSelectionWrite { content: text }));
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
		let items = vec![ApplicationDataItem::Resource(ApplicationDataResource {
			id,
			hash: Some(hash),
			sources: vec![DataSource::Embedded],
			data: Some(ResourceData(bytes)),
		})];
		let payload = TransferData::ApplicationData(items);

		editor.paste_from_clipboard(payload.into()).await;

		let registry = &editor.active_document().resources.registry;
		assert!(registry.contains(&id), "the carried resource should be registered after paste");
		assert_eq!(registry.hash(&id), Some(hash), "the carried resource should resolve to the carried bytes' hash");
	}

	/// The table entry each layer in the active document's network names.
	fn layer_entries(editor: &EditorTestUtils) -> Vec<graph_craft::document::NetworkId> {
		let mut entries: Vec<_> = editor
			.active_document()
			.network_interface
			.document_network()
			.nodes
			.values()
			.filter_map(|node| match node.implementation {
				graph_craft::document::DocumentNodeImplementation::Definition(id) => Some(id),
				_ => None,
			})
			.collect();
		entries.sort();
		entries
	}

	fn assert_consistent(editor: &EditorTestUtils) {
		let violations = editor.active_document().network_interface.validate_invariants();
		assert!(violations.is_empty(), "{violations:#?}");
	}

	/// Pasting into the document a layer was copied from shares one network between the copy and the original.
	#[tokio::test]
	async fn a_pasted_copy_shares_its_network() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 100., 100.).await;

		let clipboard = copy_layers_to_clipboard(&mut editor).await;
		editor.paste_from_clipboard(clipboard.clone()).await;

		let entries = layer_entries(&editor);
		assert_eq!(entries.len(), 2);
		assert_eq!(entries[0], entries[1], "the copy should name the original's entry");
		assert_eq!(editor.active_document().network_interface.document_network().definitions.len(), 1);
		assert_consistent(&editor);
	}

	/// Pasting into another document adds the copied network under its own ID, and pasting the same content again reuses it.
	#[tokio::test]
	async fn pasting_into_another_document_carries_the_network() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 100., 100.).await;
		let copied = layer_entries(&editor);
		let clipboard = copy_layers_to_clipboard(&mut editor).await;

		editor.new_document().await;
		editor.paste_from_clipboard(clipboard.clone()).await;
		assert_eq!(layer_entries(&editor), copied);
		assert_consistent(&editor);

		editor.paste_from_clipboard(clipboard.clone()).await;
		assert_eq!(layer_entries(&editor), [copied.clone(), copied].concat(), "identical content should be reused");
		assert_eq!(editor.active_document().network_interface.document_network().definitions.len(), 1);
		assert_consistent(&editor);
	}

	/// Pasting a network that differs from the one this document holds under the same ID adds it under a fresh ID instead.
	#[tokio::test]
	async fn pasting_a_diverged_network_gives_it_a_new_id() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 100., 100.).await;
		let interface = &mut editor.active_document_mut().network_interface;
		let rectangle = interface
			.document_network()
			.nodes
			.iter()
			.find_map(|(id, node)| matches!(node.implementation, graph_craft::document::DocumentNodeImplementation::Definition(_)).then_some(*id))
			.expect("the rectangle layer");
		assert!(interface.make_unique(&rectangle, &[]), "a catalog network forks on every write, so it never diverges");
		let clipboard = copy_layers_to_clipboard(&mut editor).await;

		editor.new_document().await;
		editor.paste_from_clipboard(clipboard.clone()).await;
		let [entry] = layer_entries(&editor)[..] else { panic!("the paste should add one layer") };

		// Diverge the pasted entry from the copied one
		let interface = &mut editor.active_document_mut().network_interface;
		let layer = interface
			.document_network()
			.nodes
			.iter()
			.find_map(|(id, node)| (node.implementation == graph_craft::document::DocumentNodeImplementation::Definition(entry)).then_some(*id))
			.expect("the pasted layer");
		let (inner, input_index) = interface.document_network().definitions[&entry]
			.nodes
			.iter()
			.find_map(|(id, node)| node.inputs.iter().position(|input| matches!(input, NodeInput::Value { .. })).map(|index| (*id, index)))
			.expect("the entry should have a value input");
		let before = interface.document_network().definitions[&entry].clone();
		let NodeInput::Value { tagged_value, exposed } = &before.nodes[&inner].inputs[input_index] else {
			unreachable!("the input was found as a value")
		};
		let flipped = NodeInput::value((**tagged_value).clone(), !exposed);
		interface.set_input(&InputConnector::node_at_index(inner, input_index), flipped, &[layer]);
		assert_ne!(interface.document_network().definitions[&entry], before, "the edit should change the pasted entry");

		editor.paste_from_clipboard(clipboard.clone()).await;
		let entries = layer_entries(&editor);
		assert_eq!(entries.len(), 2);
		assert!(entries.contains(&entry));
		assert_ne!(entries[0], entries[1], "the diverged entry should not be overwritten or reused");
		assert_consistent(&editor);
	}
}
