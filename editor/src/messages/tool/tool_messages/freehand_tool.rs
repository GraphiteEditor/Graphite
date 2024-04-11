use super::tool_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_types::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_functions::path_endpoint_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::utility_functions::should_extend;
use bezier_rs::ManipulatorGroup;
use glam::DVec2;
use graph_craft::document::{value::TaggedValue, NodeId, NodeInput};
use graphene_core::uuid::generate_uuid;
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::vector::VectorModificationType;
use graphene_core::Color;
use graphene_std::vector::{PointId, SegmentId};

#[derive(Default)]
pub struct FreehandTool {
	fsm_state: FreehandToolFsmState,
	data: FreehandToolData,
	options: FreehandOptions,
}

pub struct FreehandOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
}

impl Default for FreehandOptions {
	fn default() -> Self {
		Self {
			line_weight: 5.,
			fill: ToolColorOptions::new_none(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[impl_message(Message, ToolMessage, Freehand)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FreehandToolMessage {
	// Standard messages
	Overlays(OverlayContext),
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove,
	UpdateOptions(FreehandOptionsUpdate),
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FreehandOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FreehandToolFsmState {
	#[default]
	Ready,
	Drawing,
}

impl ToolMetadata for FreehandTool {
	fn icon_name(&self) -> String {
		"VectorFreehandTool".into()
	}
	fn tooltip(&self) -> String {
		"Freehand Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Freehand
	}
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(1.)
		.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for FreehandTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorButton| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::FillColor(color.value)).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorButton| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::StrokeColor(color.value)).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for FreehandTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Freehand(FreehandToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.data, tool_data, &self.options, responses, true);
			return;
		};
		match action {
			FreehandOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
			}
			FreehandOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			FreehandOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			FreehandOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			FreehandOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			FreehandOptionsUpdate::WorkingColors(primary, secondary) => {
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
			FreehandToolFsmState::Ready => actions!(FreehandToolMessageDiscriminant;
				DragStart,
				DragStop,
			),
			FreehandToolFsmState::Drawing => actions!(FreehandToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Abort,
			),
		}
	}
}

impl ToolTransition for FreehandTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			overlay_provider: Some(|overlay_context: OverlayContext| FreehandToolMessage::Overlays(overlay_context).into()),
			tool_abort: Some(FreehandToolMessage::Abort.into()),
			working_color_changed: Some(FreehandToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct FreehandToolData {
	positions: Vec<DVec2>,
	dragged: bool,
	required_tangent: DVec2,
	last_tangent: DVec2,

	start: Option<(PointId, DVec2)>,
	end: Option<(PointId, DVec2)>,
	segment: Option<SegmentId>,

	weight: f64,
	layer: Option<LayerNodeIdentifier>,
}

impl FreehandToolData {
	fn smooth(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		const TOLLERANCE: f64 = 5.;
		const MAX_POSITIONS: usize = 16;

		let Some(layer) = self.layer else { return };

		let determinant = document.metadata.document_to_viewport.matrix2.determinant().abs();
		let tolerance_sq = 0.02 * determinant * TOLLERANCE * TOLLERANCE * (0.2 * TOLLERANCE - 2.).exp();

		let fit = if self.positions.len() < MAX_POSITIONS {
			bezier_rs::Subpath::<PointId>::fit_cubic(&self.positions, 1, self.required_tangent, tolerance_sq)
		} else {
			None
		};

		let Some(bezier) = fit.and_then(|subpath| subpath.iter().next()) else {
			// Start a new segment
			if let Some(point) = self.end {
				self.positions.clear();
				self.positions.push(point.1);
				self.start = Some(point);
				self.end = None;
				self.required_tangent = self.last_tangent;
				self.segment = None;
			}
			return;
		};

		let set_point = |point: &mut Option<(PointId, DVec2)>, pos: DVec2, layer, responses: &mut VecDeque<Message>| {
			if let Some((id, current)) = point {
				let delta = pos - *current;
				*current += delta;
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification_type: VectorModificationType::ApplyDelta { point: *id, delta },
				});
				*id
			} else {
				let id = PointId::generate();
				*point = Some((id, pos));
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification_type: VectorModificationType::InsertPoint { id, pos },
				});
				id
			}
		};

		let start = set_point(&mut self.start, bezier.start, layer, responses);
		let end = set_point(&mut self.end, bezier.end, layer, responses);

		let handles = bezier.handles;
		if let Some(segment) = self.segment {
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification_type: VectorModificationType::SetHandles { segment, handles },
			});
		} else {
			let id = SegmentId::generate();
			self.segment = Some(id);
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification_type: VectorModificationType::InsertSegment { id, start, end, handles },
			});
		}

		self.last_tangent = match bezier.handles {
			bezier_rs::BezierHandles::Linear => bezier.end - bezier.start,
			bezier_rs::BezierHandles::Quadratic { handle } => bezier.end - handle,
			bezier_rs::BezierHandles::Cubic { handle_end, .. } => bezier.end - handle_end,
		}
		.normalize_or_zero();
	}

	fn push(&mut self, document: &DocumentMessageHandler, layer: LayerNodeIdentifier, viewport: DVec2) {
		let transform = document.metadata().transform_to_viewport(layer);
		let pos = transform.inverse().transform_point2(viewport);

		if self.positions.last() != Some(&pos) && pos.is_finite() {
			self.positions.push(pos);
		}
	}
}

impl Fsm for FreehandToolFsmState {
	type ToolData = FreehandToolData;
	type ToolOptions = FreehandOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			shape_editor,
			..
		} = tool_action_data;

		let ToolMessage::Freehand(event) = event else {
			return self;
		};
		match (self, event) {
			(_, FreehandToolMessage::Overlays(mut overlay_context)) => {
				path_endpoint_overlays(document, shape_editor, &mut overlay_context);

				self
			}
			(FreehandToolFsmState::Ready, FreehandToolMessage::DragStart) => {
				responses.add(DocumentMessage::StartTransaction);

				tool_data.weight = tool_options.line_weight;
				tool_data.positions.clear();
				tool_data.required_tangent = DVec2::ZERO;
				tool_data.last_tangent = DVec2::ZERO;
				tool_data.start = None;
				tool_data.end = None;
				tool_data.segment = None;
				tool_data.dragged = false;

				let layer = if let Some((layer, point, pos)) = should_extend(document, input.mouse.position, crate::consts::SNAP_POINT_TOLERANCE) {
					tool_data.positions.push(pos);
					tool_data.start = Some((point, pos));
					tool_data.layer = Some(layer);
					layer
				} else {
					responses.add(DocumentMessage::DeselectAllLayers);

					let parent = document.new_layer_parent();

					let nodes = {
						let node_type = resolve_document_node_type("Path Modify").expect("Path Modify node does not exist");
						let node = node_type.to_document_node_default_inputs([], Default::default());

						HashMap::from([(NodeId(0), node)])
					};

					let layer = graph_modification_utils::new_custom(NodeId(generate_uuid()), nodes, parent, responses);
					tool_options.fill.apply_fill(layer, responses);
					tool_options.stroke.apply_stroke(tool_data.weight, layer, responses);

					tool_data.layer = Some(layer);
					parent
				};
				tool_data.push(&document, layer, input.mouse.position);
				tool_data.smooth(document, responses);

				FreehandToolFsmState::Drawing
			}
			(FreehandToolFsmState::Drawing, FreehandToolMessage::PointerMove) => {
				if let Some(layer) = tool_data.layer {
					tool_data.push(&document, layer, input.mouse.position);
					tool_data.smooth(document, responses);
					tool_data.dragged = true;
				}

				FreehandToolFsmState::Drawing
			}
			(FreehandToolFsmState::Drawing, FreehandToolMessage::DragStop) => {
				if tool_data.dragged {
					responses.add(DocumentMessage::CommitTransaction);
				} else {
					responses.add(DocumentMessage::DocumentHistoryBackward);
				}

				tool_data.layer = None;

				FreehandToolFsmState::Ready
			}
			(FreehandToolFsmState::Drawing, FreehandToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.layer = None;

				FreehandToolFsmState::Ready
			}
			(_, FreehandToolMessage::WorkingColorChanged) => {
				responses.add(FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::WorkingColors(
					Some(global_tool_data.primary_color),
					Some(global_tool_data.secondary_color),
				)));
				self
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			FreehandToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Draw Polyline")])]),
			FreehandToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}
