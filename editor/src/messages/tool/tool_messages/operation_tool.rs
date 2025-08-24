use super::tool_prelude::*;
use crate::consts::{DEFAULT_STROKE_WIDTH, DRAG_THRESHOLD, PATH_JOIN_THRESHOLD, SNAP_POINT_TOLERANCE};
use crate::messages::input_mapper::utility_types::input_mouse::MouseKeys;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_functions::path_endpoint_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, find_spline, merge_layers, merge_points};
use crate::messages::tool::common_functionality::shapes::shape_utility::{extract_circular_repeat_parameters, extract_star_parameters};
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapManager, SnapTypeConfiguration, SnappedPoint};
use crate::messages::tool::common_functionality::utility_functions::{closest_point, should_extend};
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::Color;
use graphene_std::vector::{PointId, SegmentId, VectorModificationType};

#[derive(Default, ExtractField)]
pub struct OperationTool {
	fsm_state: OperationToolFsmState,
	tool_data: OperationToolData,
	options: OperationOptions,
}

pub struct OperationOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
}

impl Default for OperationOptions {
	fn default() -> Self {
		Self {
			line_weight: DEFAULT_STROKE_WIDTH,
			fill: ToolColorOptions::new_none(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[impl_message(Message, ToolMessage, Operation)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum OperationToolMessage {
	// Standard messages
	Overlays { context: OverlayContext },
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	Confirm,
	DragStart,
	DragStop,
	PointerMove,
	PointerOutsideViewport,
	Undo,
	UpdateOptions { options: OperationOptionsUpdate },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum OperationToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum OperationOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
}

impl ToolMetadata for OperationTool {
	fn icon_name(&self) -> String {
		"GeneralOperationTool".into()
	}
	fn tooltip(&self) -> String {
		"Operation Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Operation
	}
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| {
			OperationToolMessage::UpdateOptions {
				options: OperationOptionsUpdate::LineWeight(number_input.value.unwrap()),
			}
			.into()
		})
		.widget_holder()
}

impl LayoutHolder for OperationTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| {
				OperationToolMessage::UpdateOptions {
					options: OperationOptionsUpdate::FillColor(None),
				}
				.into()
			},
			|color_type: ToolColorType| {
				WidgetCallback::new(move |_| {
					OperationToolMessage::UpdateOptions {
						options: OperationOptionsUpdate::FillColorType(color_type.clone()),
					}
					.into()
				})
			},
			|color: &ColorInput| {
				OperationToolMessage::UpdateOptions {
					options: OperationOptionsUpdate::FillColor(color.value.as_solid().map(|color| color.to_linear_srgb())),
				}
				.into()
			},
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| {
				OperationToolMessage::UpdateOptions {
					options: OperationOptionsUpdate::StrokeColor(None),
				}
				.into()
			},
			|color_type: ToolColorType| {
				WidgetCallback::new(move |_| {
					OperationToolMessage::UpdateOptions {
						options: OperationOptionsUpdate::StrokeColorType(color_type.clone()),
					}
					.into()
				})
			},
			|color: &ColorInput| {
				OperationToolMessage::UpdateOptions {
					options: OperationOptionsUpdate::StrokeColor(color.value.as_solid().map(|color| color.to_linear_srgb())),
				}
				.into()
			},
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for OperationTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		let ToolMessage::Operation(OperationToolMessage::UpdateOptions { options }) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, context, &self.options, responses, true);
			return;
		};
		match options {
			OperationOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			OperationOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
			}
			OperationOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			OperationOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			OperationOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			OperationOptionsUpdate::WorkingColors(primary, secondary) => {
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
			OperationToolFsmState::Ready => actions!(OperationToolMessageDiscriminant;
				Undo,
				DragStart,
				DragStop,
				PointerMove,
				Confirm,
				Abort,
			),
			OperationToolFsmState::Drawing => actions!(OperationToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Confirm,
				Abort,
			),
		}
	}
}

impl ToolTransition for OperationTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			overlay_provider: Some(|context: OverlayContext| OperationToolMessage::Overlays { context }.into()),
			tool_abort: Some(OperationToolMessage::Abort.into()),
			working_color_changed: Some(OperationToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct OperationToolData {
	drag_start: DVec2,
	clicked_layer: LayerNodeIdentifier,
	layers_dragging: Vec<(LayerNodeIdentifier, f64)>,
	initial_center: DVec2,
}

impl OperationToolData {
	fn cleanup(&mut self) {
		self.layers_dragging.clear();
	}

	fn modify_circular_repeat(&mut self) {}
}

impl Fsm for OperationToolFsmState {
	type ToolData = OperationToolData;
	type ToolOptions = OperationOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		tool_action_data: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext {
			document,
			global_tool_data,
			input,
			shape_editor,
			preferences,
			..
		} = tool_action_data;

		let ToolMessage::Operation(event) = event else { return self };
		match (self, event) {
			(_, OperationToolMessage::Overlays { context: mut overlay_context }) => {}
			(OperationToolFsmState::Ready, OperationToolMessage::DragStart) => {
				let selected_layers = document
					.network_interface
					.selected_nodes()
					.selected_layers(document.metadata())
					.collect::<HashSet<LayerNodeIdentifier>>();
				let Some(layer) = document.click(&input) else { return self };
				let viewport = document.metadata().transform_to_viewport(layer);

				if selected_layers.contains(&layer) {
					// store all
					tool_data.layers_dragging = selected_layers
						.iter()
						.map(|layer| {
							let (_angle_offset, radius, _count) = extract_circular_repeat_parameters(Some(*layer), document).unwrap_or((0.0, 0.0, 6));
							(*layer, radius)
						})
						.collect::<Vec<(LayerNodeIdentifier, f64)>>();
				} else {
					// deselect all the layer and store the clicked layer for repeat and dragging

					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });
					let (_angle_offset, radius, _count) = extract_circular_repeat_parameters(Some(layer), document).unwrap_or((0.0, 0.0, 6));
					tool_data.layers_dragging = vec![(layer, radius)];
				}
				tool_data.drag_start = input.mouse.position;
				tool_data.clicked_layer = layer;
				tool_data.initial_center = viewport.transform_point2(DVec2::ZERO);

				OperationToolFsmState::Drawing
			}
			(OperationToolFsmState::Drawing, OperationToolMessage::DragStop) => {
				tool_data.cleanup();
				OperationToolFsmState::Ready
			}
			(OperationToolFsmState::Drawing, OperationToolMessage::PointerMove) => {
				// Don't add the repeat node unless dragging more that 5 px
				if tool_data.drag_start.distance(input.mouse.position) < 5. {
					return self;
				};

				let viewport = document.metadata().transform_to_viewport(tool_data.clicked_layer);
				let center = viewport.transform_point2(DVec2::ZERO);
				let sign = (input.mouse.position - tool_data.initial_center).dot(tool_data.drag_start - tool_data.initial_center).signum();
				let delta = viewport.inverse().transform_vector2(input.mouse.position - tool_data.drag_start).length() * sign;
				log::info!("{:?}", delta);
				for (layer, initial_radius) in &tool_data.layers_dragging {
					responses.add(GraphOperationMessage::CircularRepeatSet {
						layer: *layer,
						angle: 0.,
						radius: initial_radius + delta,
						count: 6,
					});
				}
				responses.add(NodeGraphMessage::RunDocumentGraph);

				OperationToolFsmState::Drawing
			}
			(_, OperationToolMessage::PointerMove) => self,

			(OperationToolFsmState::Drawing, OperationToolMessage::PointerOutsideViewport) => OperationToolFsmState::Drawing,
			(state, OperationToolMessage::PointerOutsideViewport) => state,
			(OperationToolFsmState::Drawing, OperationToolMessage::Abort) => OperationToolFsmState::Ready,
			(_, OperationToolMessage::WorkingColorChanged) => self,
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			OperationToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::Lmb, "Draw Spline"),
				HintInfo::keys([Key::Shift], "Append to Selected Layer").prepend_plus(),
			])]),
			OperationToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Extend Spline")]),
				HintGroup(vec![HintInfo::keys([Key::Enter], "End Spline")]),
			]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}
