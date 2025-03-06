use super::tool_prelude::*;
use crate::consts::DEFAULT_STROKE_WIDTH;
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::transformation::Selected;
use crate::messages::portfolio::document::{graph_operation::utility_types::TransformIn, overlays::utility_types::OverlayContext, utility_types::network_interface::InputConnector};
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::pivot::Pivot;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::common_functionality::snapping::{self, SnapCandidatePoint, SnapData};
use crate::messages::tool::common_functionality::transformation_cage::*;

use graph_craft::document::{value::TaggedValue, NodeId, NodeInput};
use graphene_core::renderer::Quad;
use graphene_core::Color;

#[derive(Default)]
pub struct RectangleTool {
	fsm_state: RectangleToolFsmState,
	tool_data: RectangleToolData,
	options: RectangleToolOptions,
}

pub struct RectangleToolOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
}

impl Default for RectangleToolOptions {
	fn default() -> Self {
		Self {
			line_weight: DEFAULT_STROKE_WIDTH,
			fill: ToolColorOptions::new_secondary(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum RectangleOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
}

#[impl_message(Message, ToolMessage, Rectangle)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum RectangleToolMessage {
	// Standard messages
	Overlays(OverlayContext),
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove { center: Key, lock_ratio: Key },
	PointerOutsideViewport { center: Key, lock_ratio: Key },
	UpdateOptions(RectangleOptionsUpdate),
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| RectangleToolMessage::UpdateOptions(RectangleOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for RectangleTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| RectangleToolMessage::UpdateOptions(RectangleOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| RectangleToolMessage::UpdateOptions(RectangleOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorInput| RectangleToolMessage::UpdateOptions(RectangleOptionsUpdate::FillColor(color.value.as_solid())).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| RectangleToolMessage::UpdateOptions(RectangleOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| RectangleToolMessage::UpdateOptions(RectangleOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorInput| RectangleToolMessage::UpdateOptions(RectangleOptionsUpdate::StrokeColor(color.value.as_solid())).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for RectangleTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Rectangle(RectangleToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
			return;
		};
		match action {
			RectangleOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
			}
			RectangleOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			RectangleOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			RectangleOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			RectangleOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			RectangleOptionsUpdate::WorkingColors(primary, secondary) => {
				self.options.stroke.primary_working_color = primary;
				self.options.stroke.secondary_working_color = secondary;
				self.options.fill.primary_working_color = primary;
				self.options.fill.secondary_working_color = secondary;
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			RectangleToolFsmState::Ready => actions!(RectangleToolMessageDiscriminant;
				DragStart,
				PointerMove,
			),
			_ => actions!(RectangleToolMessageDiscriminant;
				DragStop,
				Abort,
				PointerMove,
			),
		}
	}
}

impl ToolMetadata for RectangleTool {
	fn icon_name(&self) -> String {
		"VectorRectangleTool".into()
	}
	fn tooltip(&self) -> String {
		"Rectangle Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Rectangle
	}
}

impl ToolTransition for RectangleTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			overlay_provider: Some(|overlay_context| RectangleToolMessage::Overlays(overlay_context).into()),
			tool_abort: Some(RectangleToolMessage::Abort.into()),
			working_color_changed: Some(RectangleToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RectangleToolFsmState {
	#[default]
	Ready,
	Drawing,
	DraggingPivot,
	ResizingBounds,
}

#[derive(Clone, Debug, Default)]
struct RectangleToolData {
	data: Resize,
	auto_panning: AutoPanning,
	layer: Option<LayerNodeIdentifier>,
	/// The transform of the rectangle layer in document space at the start of the transformation.
	original_transform: DAffine2,
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	pivot: Pivot,
	cursor: MouseCursorIcon,
	snap_candidates: Vec<SnapCandidatePoint>,
	bounding_box_manager: Option<BoundingBoxManager>,
}

impl RectangleToolData {
	fn get_snap_candidates(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) {
		self.snap_candidates.clear();

		if let Some(layer) = self.layer {
			snapping::get_layer_snap_points(layer, &SnapData::new(document, input), &mut self.snap_candidates);
			if let Some(bounds) = document.metadata().bounding_box_with_transform(layer, DAffine2::IDENTITY) {
				let quad = document.metadata().transform_to_document(layer) * Quad::from_box(bounds);
				snapping::get_bbox_points(quad, &mut self.snap_candidates, snapping::BBoxSnapValues::BOUNDING_BOX, document);
			}
		}
	}
}

impl Fsm for RectangleToolFsmState {
	type ToolData = RectangleToolData;
	type ToolOptions = RectangleToolOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionHandlerData {
			document, global_tool_data, input, ..
		}: &mut ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let shape_data = &mut tool_data.data;

		let ToolMessage::Rectangle(event) = event else { return self };
		match (self, event) {
			(_, RectangleToolMessage::Overlays(mut overlay_context)) => {
				shape_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);

				let layer = document
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_visible_and_unlocked_layers(&document.network_interface)
					.find(|layer| graph_modification_utils::get_rectangle_id(*layer, &document.network_interface).is_some());
				let transform = layer.map(|layer| document.metadata().transform_to_viewport(layer)).unwrap_or(DAffine2::IDENTITY);
				let bounds = layer.and_then(|layer| document.metadata().bounding_box_with_transform(layer, DAffine2::IDENTITY));

				if let Some(bounds) = bounds {
					let bounding_box_manager = tool_data.bounding_box_manager.get_or_insert(BoundingBoxManager::default());

					bounding_box_manager.bounds = bounds;
					bounding_box_manager.transform = transform;

					bounding_box_manager.render_overlays(&mut overlay_context);

					tool_data.layer = layer;
				} else {
					tool_data.bounding_box_manager.take();
				}

				let angle = bounds
					.map(|bounds| transform * Quad::from_box(bounds))
					.map_or(0., |quad| (quad.top_left() - quad.top_right()).to_angle());

				// Update pivot
				tool_data.pivot.update_pivot(document, &mut overlay_context, angle);

				self
			}
			(RectangleToolFsmState::Ready, RectangleToolMessage::DragStart) => {
				tool_data.drag_start = input.mouse.position;
				tool_data.drag_current = input.mouse.position;

				let dragging_bounds = tool_data.bounding_box_manager.as_mut().and_then(|bounding_box| {
					let edges = bounding_box.check_selected_edges(input.mouse.position);

					bounding_box.selected_edges = edges.map(|(top, bottom, left, right)| {
						let selected_edges = SelectedEdges::new(top, bottom, left, right, bounding_box.bounds);
						bounding_box.opposite_pivot = selected_edges.calculate_pivot();
						selected_edges
					});

					edges
				});

				// Determine the state based on the mouse position
				// If the mouse is over the pivot, we are dragging the pivot this gets number one priority
				let state = if tool_data.pivot.is_over(input.mouse.position) {
					responses.add(DocumentMessage::StartTransaction);

					RectangleToolFsmState::DraggingPivot
				}
				// If the bounds are dragged, then the user is trying to resize
				else if dragging_bounds.is_some() {
					responses.add(DocumentMessage::StartTransaction);

					if let Some(bounds) = &mut tool_data.bounding_box_manager {
						bounds.original_bound_transform = bounds.transform;
						let layer = tool_data.layer.unwrap();
						tool_data.original_transform = document.metadata().transform_to_document(layer);

						let selected = [layer];
						let mut selected = Selected::new(
							&mut bounds.original_transforms,
							&mut bounds.center_of_transformation,
							&selected,
							responses,
							&document.network_interface,
							None,
							&ToolType::Rectangle,
							None,
						);
						bounds.center_of_transformation = selected.mean_average_of_pivots();
					}
					tool_data.get_snap_candidates(document, input);

					RectangleToolFsmState::ResizingBounds
				}
				// Finally if nothing else, the user is trying to draw a new shape
				else {
					shape_data.start(document, input);

					responses.add(DocumentMessage::StartTransaction);

					let node_type = resolve_document_node_type("Rectangle").expect("Rectangle node does not exist");
					let node = node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(1.), false)), Some(NodeInput::value(TaggedValue::F64(1.), false))]);
					let nodes = vec![(NodeId(0), node)];

					let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, document.new_layer_bounding_artboard(input), responses);
					responses.add(Message::StartBuffer);
					responses.add(GraphOperationMessage::TransformSet {
						layer,
						transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., input.mouse.position),
						transform_in: TransformIn::Viewport,
						skip_rerender: false,
					});
					tool_options.fill.apply_fill(layer, responses);
					tool_options.stroke.apply_stroke(tool_options.line_weight, layer, responses);
					shape_data.layer = Some(layer);

					RectangleToolFsmState::Drawing
				};

				state
			}
			(RectangleToolFsmState::Ready, RectangleToolMessage::PointerMove { .. }) => {
				shape_data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
				let mut cursor = tool_data.bounding_box_manager.as_ref().map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, true));

				// Dragging the pivot overrules the other operations
				if tool_data.pivot.is_over(input.mouse.position) {
					cursor = MouseCursorIcon::Move;
				}

				if tool_data.cursor != cursor {
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				responses.add(OverlaysMessage::Draw);

				RectangleToolFsmState::Ready
			}
			(RectangleToolFsmState::Drawing, RectangleToolMessage::PointerMove { center, lock_ratio }) => {
				if let Some([start, end]) = shape_data.calculate_points(document, input, center, lock_ratio) {
					if let Some(layer) = shape_data.layer {
						let Some(node_id) = graph_modification_utils::get_rectangle_id(layer, &document.network_interface) else {
							return self;
						};

						responses.add(NodeGraphMessage::SetInput {
							input_connector: InputConnector::node(node_id, 1),
							input: NodeInput::value(TaggedValue::F64((start.x - end.x).abs()), false),
						});
						responses.add(NodeGraphMessage::SetInput {
							input_connector: InputConnector::node(node_id, 2),
							input: NodeInput::value(TaggedValue::F64((start.y - end.y).abs()), false),
						});
						responses.add(GraphOperationMessage::TransformSet {
							layer,
							transform: DAffine2::from_translation((start + end) / 2.),
							transform_in: TransformIn::Local,
							skip_rerender: false,
						});
					}
				}

				// Auto-panning
				let messages = [
					RectangleToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
					RectangleToolMessage::PointerMove { center, lock_ratio }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				self
			}
			(RectangleToolFsmState::DraggingPivot, RectangleToolMessage::PointerMove { center, lock_ratio }) => {
				let mouse_position = input.mouse.position;
				let snapped_mouse_position = mouse_position;
				tool_data.pivot.set_viewport_position(snapped_mouse_position, document, responses);

				// AutoPanning
				let messages = [
					RectangleToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
					RectangleToolMessage::PointerMove { center, lock_ratio }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				RectangleToolFsmState::DraggingPivot
			}
			(RectangleToolFsmState::ResizingBounds, RectangleToolMessage::PointerMove { center, lock_ratio }) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					if let Some(edges) = &mut bounds.selected_edges {
						let Some(layer) = tool_data.layer else { return RectangleToolFsmState::Ready };
						let node_id = graph_modification_utils::get_rectangle_id(layer, &document.network_interface).unwrap();

						let (center, lock_ratio) = (input.keyboard.key(center), input.keyboard.key(lock_ratio));

						let ignore = [layer];
						let center = center.then_some(bounds.center_of_transformation);
						let snap = Some(SizeSnapData {
							manager: &mut shape_data.snap_manager,
							points: &mut tool_data.snap_candidates,
							snap_data: SnapData::ignore(document, input, &ignore),
						});

						let (position, size) = edges.new_size(input.mouse.position, bounds.original_bound_transform, center, lock_ratio, snap);
						// Normalise so the size is always positive
						let (position, size) = (position + size / 2., size.abs());

						// Compute the offset needed for the top left in bounds space
						let original_position = (edges.bounds[0] + edges.bounds[1]) / 2.;
						let translation_bounds_space = position - original_position;

						// Compute a transformation from bounds->viewport->layer
						let transform_to_layer = document.metadata().transform_to_viewport(layer).inverse() * bounds.original_bound_transform;
						let size_layer = transform_to_layer.transform_vector2(size);

						// Find the translation necessary from the original position in viewport space
						let translation_viewport = bounds.original_bound_transform.transform_vector2(translation_bounds_space);

						responses.add(NodeGraphMessage::SetInput {
							input_connector: InputConnector::node(node_id, 1),
							input: NodeInput::value(TaggedValue::F64(size_layer.x), false),
						});
						responses.add(NodeGraphMessage::SetInput {
							input_connector: InputConnector::node(node_id, 2),
							input: NodeInput::value(TaggedValue::F64(size_layer.y), false),
						});
						responses.add(GraphOperationMessage::TransformSet {
							layer: layer,
							transform: DAffine2::from_translation(translation_viewport) * document.metadata().document_to_viewport * tool_data.original_transform,
							transform_in: TransformIn::Viewport,
							skip_rerender: false,
						});
					}
				}

				self
			}
			(RectangleToolFsmState::Drawing, RectangleToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				RectangleToolFsmState::Drawing
			}
			(state, RectangleToolMessage::PointerOutsideViewport { center, lock_ratio }) => {
				// Auto-panning
				let messages = [
					RectangleToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
					RectangleToolMessage::PointerMove { center, lock_ratio }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(RectangleToolFsmState::Drawing, RectangleToolMessage::DragStop) => {
				input.mouse.finish_transaction(shape_data.viewport_drag_start(document), responses);
				shape_data.cleanup(responses);

				RectangleToolFsmState::Ready
			}
			(RectangleToolFsmState::DraggingPivot, RectangleToolMessage::DragStop) => {
				let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
					true => DocumentMessage::AbortTransaction,
					false => DocumentMessage::EndTransaction,
				};
				responses.add(response);

				shape_data.snap_manager.cleanup(responses);

				RectangleToolFsmState::Ready
			}
			(RectangleToolFsmState::ResizingBounds, RectangleToolMessage::DragStop) => {
				let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
					true => DocumentMessage::AbortTransaction,
					false => DocumentMessage::EndTransaction,
				};
				responses.add(response);

				shape_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				RectangleToolFsmState::Ready
			}
			(RectangleToolFsmState::Drawing, RectangleToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);

				shape_data.cleanup(responses);

				RectangleToolFsmState::Ready
			}
			(_, RectangleToolMessage::Abort) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				responses.add(DocumentMessage::AbortTransaction);
				shape_data.snap_manager.cleanup(responses);
				responses.add(OverlaysMessage::Draw);

				RectangleToolFsmState::Ready
			}
			(_, RectangleToolMessage::WorkingColorChanged) => {
				responses.add(RectangleToolMessage::UpdateOptions(RectangleOptionsUpdate::WorkingColors(
					Some(global_tool_data.primary_color),
					Some(global_tool_data.secondary_color),
				)));
				self
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>, _tool_data: &Self::ToolData) {
		let hint_data = match self {
			RectangleToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Rectangle"),
				HintInfo::keys([Key::Shift], "Constrain Square").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
			])]),
			RectangleToolFsmState::Drawing | RectangleToolFsmState::ResizingBounds => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")]),
			]),
			RectangleToolFsmState::DraggingPivot => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}
