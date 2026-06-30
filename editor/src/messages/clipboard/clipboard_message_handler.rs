use crate::consts::DEFAULT_STROKE_WIDTH;
use crate::messages::clipboard::utility_types::{ClipboardContent, ClipboardContentRaw};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_network_node_type;
use crate::messages::portfolio::document::utility_types::clipboards::CopyBufferEntry;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface;
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::utility_types::ToolType;
use glam::DAffine2;
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::raster::Image;
use graphene_std::subpath::BezierHandles;
use graphene_std::vector::misc::HandleId;
use graphene_std::vector::{PointId, SegmentId, Vector, VectorModificationType};
use graphite_proc_macros::{ExtractField, message_handler_data};

const CLIPBOARD_PREFIX_LAYER: &str = "graphite/layer: ";
const CLIPBOARD_PREFIX_NODES: &str = "graphite/nodes: ";
const CLIPBOARD_PREFIX_VECTOR: &str = "graphite/vector: ";

#[derive(ExtractField)]
pub struct ClipboardMessageContext<'a> {
	pub portfolio: &'a mut PortfolioMessageHandler,
	pub current_tool: &'a ToolType,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct ClipboardMessageHandler {}

#[message_handler_data]
impl MessageHandler<ClipboardMessage, ClipboardMessageContext<'_>> for ClipboardMessageHandler {
	fn process_message(&mut self, message: ClipboardMessage, responses: &mut std::collections::VecDeque<Message>, context: ClipboardMessageContext) {
		let ClipboardMessageContext { portfolio, current_tool } = context;

		match message {
			// Frontend/system-clipboard boundary
			ClipboardMessage::Cut => responses.add(FrontendMessage::TriggerSelectionRead { cut: true }),
			ClipboardMessage::Copy => responses.add(FrontendMessage::TriggerSelectionRead { cut: false }),
			ClipboardMessage::Paste => responses.add(FrontendMessage::TriggerClipboardRead),
			ClipboardMessage::ReadClipboard { content } => match content {
				ClipboardContentRaw::Text(text) => {
					if let Some(layer) = text.strip_prefix(CLIPBOARD_PREFIX_LAYER) {
						responses.add(ClipboardMessage::PasteSerializedData { data: layer.to_string() });
					} else if let Some(nodes) = text.strip_prefix(CLIPBOARD_PREFIX_NODES) {
						responses.add(NodeGraphMessage::PasteNodes { serialized_nodes: nodes.to_string() });
					} else if let Some(vector) = text.strip_prefix(CLIPBOARD_PREFIX_VECTOR) {
						responses.add(ClipboardMessage::PasteSerializedVector { data: vector.to_string() });
					} else {
						responses.add(FrontendMessage::TriggerSelectionWrite { content: text });
					}
				}
				ClipboardContentRaw::Svg(svg) => {
					responses.add(PortfolioMessage::PasteSvg {
						svg,
						name: None,
						mouse: None,
						parent_and_insert_index: None,
					});
				}
				ClipboardContentRaw::Image { data, width, height } => {
					responses.add(PortfolioMessage::PasteImage {
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
					ClipboardContent::Layer(layer) => format!("{CLIPBOARD_PREFIX_LAYER}{layer}"),
					ClipboardContent::Nodes(nodes) => format!("{CLIPBOARD_PREFIX_NODES}{nodes}"),
					ClipboardContent::Vector(vector) => format!("{CLIPBOARD_PREFIX_VECTOR}{vector}"),
					ClipboardContent::Text(text) => text,
				};
				responses.add(FrontendMessage::TriggerClipboardWrite { content: text });
			}

			// Graphite-native copy/paste logic
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

					buffer.push(CopyBufferEntry {
						nodes: active_document.network_interface.copy_nodes(&copy_ids, &[]).collect(),
						selected: active_document.network_interface.selected_nodes().selected_layers_contains(layer, active_document.metadata()),
						visible: active_document.network_interface.selected_nodes().layer_visible(layer, &active_document.network_interface),
						locked: active_document.network_interface.selected_nodes().layer_locked(layer, &active_document.network_interface),
						collapsed: false,
					});
				}

				let Ok(data) = serde_json::to_string(&buffer) else {
					log::error!("Failed to serialize nodes for clipboard");
					return;
				};
				responses.add(ClipboardMessage::Write {
					content: ClipboardContent::Layer(data),
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
			ClipboardMessage::PasteSerializedData { data } => {
				if let Some(document) = portfolio.active_document() {
					let mut all_new_ids = Vec::new();
					if let Ok(data) = serde_json::from_str::<Vec<CopyBufferEntry>>(&data) {
						let parent = document.new_layer_parent(false);
						let mut layers = Vec::new();

						let mut added_nodes = false;

						for entry in data.into_iter().rev() {
							if !added_nodes {
								responses.add(DocumentMessage::DeselectAllLayers);
								responses.add(DocumentMessage::AddTransaction);
								added_nodes = true;
							}

							let new_ids: HashMap<_, _> = entry.nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect();
							let layer = LayerNodeIdentifier::new_unchecked(new_ids[&NodeId(0)]);
							all_new_ids.extend(new_ids.values().cloned());

							responses.add(NodeGraphMessage::AddNodes { nodes: entry.nodes, new_ids });
							responses.add(NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index: 0 });
							layers.push(layer);
						}

						responses.add(NodeGraphMessage::RunDocumentGraph);
						responses.add(NodeGraphMessage::SelectedNodesSet { nodes: all_new_ids });
						responses.add(DeferMessage::AfterGraphRun {
							messages: vec![PortfolioMessage::CenterPastedLayers { layers }.into()],
						});
					}
				}
			}
			// Custom paste implementation for Path tool
			ClipboardMessage::PasteSerializedVector { data } => {
				// If using Path tool then send the operation to Path tool
				if *current_tool == ToolType::Path {
					responses.add(PathToolMessage::Paste { data });
					return;
				}

				// If not using Path tool, create new layers and add paths into those
				if let Some(document) = portfolio.active_document() {
					let Ok(data) = serde_json::from_str::<Vec<(LayerNodeIdentifier, Vector, DAffine2)>>(&data) else {
						return;
					};

					let mut layers = Vec::new();

					for (_, new_vector, transform) in data {
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
							let new_segment_id = SegmentId::generate();

							segments_map.insert(segment_id, new_segment_id);

							let handles = match bezier.handles {
								BezierHandles::Linear => [None, None],
								BezierHandles::Quadratic { handle } => [Some(handle - bezier.start), None],
								BezierHandles::Cubic { handle_start, handle_end } => [Some(handle_start - bezier.start), Some(handle_end - bezier.end)],
							};

							let points = [points_map[&start], points_map[&end]];
							let modification_type = VectorModificationType::InsertSegment { id: new_segment_id, points, handles };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}

						// Set G1 continuity
						for handles in new_vector.colinear_manipulators {
							let to_new_handle = |handle: HandleId| -> HandleId {
								HandleId {
									ty: handle.ty,
									segment: segments_map[&handle.segment],
								}
							};
							let new_handles = [to_new_handle(handles[0]), to_new_handle(handles[1])];
							let modification_type = VectorModificationType::SetG1Continuous { handles: new_handles, enabled: true };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}
					}

					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(Message::Defer(DeferMessage::AfterGraphRun {
						messages: vec![PortfolioMessage::CenterPastedLayers { layers }.into()],
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
