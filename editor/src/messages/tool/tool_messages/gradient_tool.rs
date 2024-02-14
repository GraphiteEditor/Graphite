use super::tool_prelude::*;
use crate::consts::{LINE_ROTATE_SNAP_ANGLE, MANIPULATOR_GROUP_MARKER_SIZE, SELECTION_THRESHOLD};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::graph_modification_utils::get_gradient;
use crate::messages::tool::common_functionality::snapping::SnapManager;

use graphene_core::vector::style::{Fill, Gradient, GradientType};

#[derive(Default)]
pub struct GradientTool {
	fsm_state: GradientToolFsmState,
	data: GradientToolData,
	options: GradientOptions,
}

#[derive(Default)]
pub struct GradientOptions {
	gradient_type: GradientType,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Gradient)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum GradientToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	Overlays(OverlayContext),

	// Tool-specific messages
	DeleteStop,
	InsertStop,
	PointerDown,
	PointerMove {
		constrain_axis: Key,
	},
	PointerUp,
	UpdateOptions(GradientOptionsUpdate),
}

#[remain::sorted]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum GradientOptionsUpdate {
	Type(GradientType),
}

impl ToolMetadata for GradientTool {
	fn icon_name(&self) -> String {
		"GeneralGradientTool".into()
	}
	fn tooltip(&self) -> String {
		"Gradient Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Gradient
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for GradientTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Gradient(GradientToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.data, tool_data, &self.options, responses, false);
			return;
		};
		match action {
			GradientOptionsUpdate::Type(gradient_type) => {
				self.options.gradient_type = gradient_type;
				if let Some(selected_gradient) = &mut self.data.selected_gradient {
					selected_gradient.gradient.gradient_type = gradient_type;
					selected_gradient.render_gradient(responses);
				}
			}
		}
	}

	advertise_actions!(GradientToolMessageDiscriminant;
		PointerDown,
		PointerUp,
		PointerMove,
		Abort,
		InsertStop,
		DeleteStop,
	);
}

impl LayoutHolder for GradientTool {
	fn layout(&self) -> Layout {
		let gradient_type = RadioInput::new(vec![
			RadioEntryData::new("Linear")
				.value("linear")
				.tooltip("Linear Gradient")
				.on_update(move |_| GradientToolMessage::UpdateOptions(GradientOptionsUpdate::Type(GradientType::Linear)).into()),
			RadioEntryData::new("Radial")
				.value("radial")
				.tooltip("Radial Gradient")
				.on_update(move |_| GradientToolMessage::UpdateOptions(GradientOptionsUpdate::Type(GradientType::Radial)).into()),
		])
		.selected_index(Some((self.selected_gradient().unwrap_or(self.options.gradient_type) == GradientType::Radial) as u32))
		.widget_holder();

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets: vec![gradient_type] }]))
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum GradientToolFsmState {
	#[default]
	Ready,
	Drawing,
}

/// Computes the transform from gradient space to viewport space (where gradient space is 0..1)
fn gradient_space_transform(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> DAffine2 {
	let bounds = document.metadata().nonzero_bounding_box(layer);
	let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);

	let multiplied = document.metadata().transform_to_viewport(layer);

	multiplied * bound_transform
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum GradientDragTarget {
	Start,
	#[default]
	End,
	Step(usize),
}

/// Contains information about the selected gradient handle
#[derive(Clone, Debug, Default)]
struct SelectedGradient {
	layer: LayerNodeIdentifier,
	transform: DAffine2,
	gradient: Gradient,
	dragging: GradientDragTarget,
}

impl SelectedGradient {
	pub fn new(gradient: Gradient, layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> Self {
		let transform = gradient_space_transform(layer, document);
		Self {
			layer,
			transform,
			gradient,
			dragging: GradientDragTarget::End,
		}
	}

	pub fn with_gradient_start(mut self, start: DVec2) -> Self {
		self.gradient.start = self.transform.inverse().transform_point2(start);
		self
	}

	pub fn update_gradient(&mut self, mut mouse: DVec2, responses: &mut VecDeque<Message>, snap_rotate: bool, gradient_type: GradientType) {
		self.gradient.gradient_type = gradient_type;

		if snap_rotate && matches!(self.dragging, GradientDragTarget::End | GradientDragTarget::Start) {
			let point = if self.dragging == GradientDragTarget::Start {
				self.transform.transform_point2(self.gradient.end)
			} else {
				self.transform.transform_point2(self.gradient.start)
			};

			let delta = point - mouse;

			let length = delta.length();
			let mut angle = -delta.angle_between(DVec2::X);

			let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
			angle = (angle / snap_resolution).round() * snap_resolution;

			let rotated = DVec2::new(length * angle.cos(), length * angle.sin());
			mouse = point - rotated;
		}

		let transformed_mouse = self.transform.inverse().transform_point2(mouse);

		match self.dragging {
			GradientDragTarget::Start => self.gradient.start = transformed_mouse,
			GradientDragTarget::End => self.gradient.end = transformed_mouse,
			GradientDragTarget::Step(s) => {
				let (start, end) = (self.transform.transform_point2(self.gradient.start), self.transform.transform_point2(self.gradient.end));

				// Calculate the new position by finding the closest point on the line
				let new_pos = ((end - start).angle_between(mouse - start)).cos() * start.distance(mouse) / start.distance(end);

				// Should not go off end but can swap
				let clamped = new_pos.clamp(0., 1.);
				self.gradient.positions[s].0 = clamped;
				let new_pos = self.gradient.positions[s];

				self.gradient.positions.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
				self.dragging = GradientDragTarget::Step(self.gradient.positions.iter().position(|x| *x == new_pos).unwrap());
			}
		}
		self.render_gradient(responses);
	}

	/// Update the layer fill to the current gradient
	pub fn render_gradient(&mut self, responses: &mut VecDeque<Message>) {
		self.gradient.transform = self.transform;
		responses.add(GraphOperationMessage::FillSet {
			layer: self.layer,
			fill: Fill::Gradient(self.gradient.clone()),
		});
	}
}

impl GradientTool {
	/// Get the gradient type of the selected gradient (if it exists)
	pub fn selected_gradient(&self) -> Option<GradientType> {
		self.data.selected_gradient.as_ref().map(|selected| selected.gradient.gradient_type)
	}
}

impl ToolTransition for GradientTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(GradientToolMessage::Abort.into()),
			overlay_provider: Some(|overlay_context| GradientToolMessage::Overlays(overlay_context).into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct GradientToolData {
	selected_gradient: Option<SelectedGradient>,
	snap_manager: SnapManager,
	drag_start: DVec2,
}

impl Fsm for GradientToolFsmState {
	type ToolData = GradientToolData;
	type ToolOptions = GradientOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document, global_tool_data, input, ..
		} = tool_action_data;

		let ToolMessage::Gradient(event) = event else {
			return self;
		};

		match (self, event) {
			(_, GradientToolMessage::Overlays(mut overlay_context)) => {
				let selected = tool_data.selected_gradient.as_ref();

				for layer in document.selected_nodes.selected_visible_layers(document.network(), document.metadata()) {
					let Some(gradient) = get_gradient(layer, &document.network) else { continue };
					let transform = gradient_space_transform(layer, document);
					let dragging = selected.filter(|selected| selected.layer == layer).map(|selected| selected.dragging);

					let Gradient { start, end, positions, .. } = gradient;
					let (start, end) = (transform.transform_point2(start), transform.transform_point2(end));

					overlay_context.line(start, end, None);
					overlay_context.manipulator_handle(start, dragging == Some(GradientDragTarget::Start));
					overlay_context.manipulator_handle(end, dragging == Some(GradientDragTarget::End));

					for (index, (position, _)) in positions.into_iter().enumerate() {
						if position.abs() < f64::EPSILON * 1000. || (1. - position).abs() < f64::EPSILON * 1000. {
							continue;
						}

						overlay_context.manipulator_handle(start.lerp(end, position), dragging == Some(GradientDragTarget::Step(index)));
					}
				}

				self
			}
			(GradientToolFsmState::Ready, GradientToolMessage::DeleteStop) => {
				let Some(selected_gradient) = &mut tool_data.selected_gradient else {
					return self;
				};

				// Skip if invalid gradient
				if selected_gradient.gradient.positions.len() < 2 {
					return self;
				}

				// Remove the selected point
				match selected_gradient.dragging {
					GradientDragTarget::Start => selected_gradient.gradient.positions.remove(0),
					GradientDragTarget::End => selected_gradient.gradient.positions.pop().unwrap(),
					GradientDragTarget::Step(index) => selected_gradient.gradient.positions.remove(index),
				};

				// The gradient has only one point and so should become a fill
				if selected_gradient.gradient.positions.len() == 1 {
					responses.add(GraphOperationMessage::FillSet {
						layer: selected_gradient.layer,
						fill: Fill::Solid(selected_gradient.gradient.positions[0].1),
					});
					return self;
				}

				// Find the minimum and maximum positions
				let min_position = selected_gradient.gradient.positions.iter().map(|(pos, _)| *pos).reduce(f64::min).expect("No min");
				let max_position = selected_gradient.gradient.positions.iter().map(|(pos, _)| *pos).reduce(f64::max).expect("No max");

				// Recompute the start and end position of the gradient (in viewport transform)
				let transform = selected_gradient.transform;
				let (start, end) = (transform.transform_point2(selected_gradient.gradient.start), transform.transform_point2(selected_gradient.gradient.end));
				let (new_start, new_end) = (start.lerp(end, min_position), start.lerp(end, max_position));
				selected_gradient.gradient.start = transform.inverse().transform_point2(new_start);
				selected_gradient.gradient.end = transform.inverse().transform_point2(new_end);

				// Remap the positions
				for (position, _) in selected_gradient.gradient.positions.iter_mut() {
					*position = (*position - min_position) / (max_position - min_position);
				}

				// Render the new gradient
				selected_gradient.render_gradient(responses);

				self
			}
			(_, GradientToolMessage::InsertStop) => {
				for layer in document.selected_nodes.selected_visible_layers(document.network(), document.metadata()) {
					let Some(mut gradient) = get_gradient(layer, &document.network) else { continue };
					let transform = gradient_space_transform(layer, document);

					let mouse = input.mouse.position;
					let (start, end) = (transform.transform_point2(gradient.start), transform.transform_point2(gradient.end));

					// Compute the distance from the mouse to the gradient line in viewport space
					let distance = (end - start).angle_between(mouse - start).sin() * (mouse - start).length();

					// If click is on the line then insert point
					if distance < (SELECTION_THRESHOLD * 2.) {
						// Try and insert the new stop
						if let Some(index) = gradient.insert_stop(mouse, transform) {
							document.backup_nonmut(responses);

							let mut selected_gradient = SelectedGradient::new(gradient, layer, document);

							// Select the new point
							selected_gradient.dragging = GradientDragTarget::Step(index);

							// Update the layer fill
							selected_gradient.render_gradient(responses);

							tool_data.selected_gradient = Some(selected_gradient);

							break;
						}
					}
				}

				self
			}
			(GradientToolFsmState::Ready, GradientToolMessage::PointerDown) => {
				let mouse = input.mouse.position;
				tool_data.drag_start = mouse;
				let tolerance = (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);

				let mut dragging = false;
				for layer in document.selected_nodes.selected_visible_layers(document.network(), document.metadata()) {
					let Some(gradient) = get_gradient(layer, &document.network) else { continue };
					let transform = gradient_space_transform(layer, document);

					// Check for dragging step
					for (index, (pos, _)) in gradient.positions.iter().enumerate() {
						let pos = transform.transform_point2(gradient.start.lerp(gradient.end, *pos));
						if pos.distance_squared(mouse) < tolerance {
							dragging = true;
							tool_data.selected_gradient = Some(SelectedGradient {
								layer,
								transform,
								gradient: gradient.clone(),
								dragging: GradientDragTarget::Step(index),
							})
						}
					}

					// Check dragging start or end handle
					for (pos, dragging_target) in [(gradient.start, GradientDragTarget::Start), (gradient.end, GradientDragTarget::End)] {
						let pos = transform.transform_point2(pos);
						if pos.distance_squared(mouse) < tolerance {
							dragging = true;
							tool_data.selected_gradient = Some(SelectedGradient {
								layer,
								transform,
								gradient: gradient.clone(),
								dragging: dragging_target,
							})
						}
					}
				}
				if dragging {
					document.backup_nonmut(responses);
					GradientToolFsmState::Drawing
				} else {
					let selected_layer = document.click(input.mouse.position, &document.network);

					// Apply the gradient to the selected layer
					if let Some(layer) = selected_layer {
						if !document.selected_nodes.selected_layers_contains(layer, document.metadata()) {
							let nodes = vec![layer.to_node()];

							responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
						}

						responses.add(DocumentMessage::StartTransaction);

						// Use the already existing gradient if it exists
						let gradient = if let Some(gradient) = get_gradient(layer, &document.network) {
							gradient.clone()
						} else {
							// Generate a new gradient
							Gradient::new(
								DVec2::ZERO,
								global_tool_data.secondary_color,
								DVec2::ONE,
								global_tool_data.primary_color,
								DAffine2::IDENTITY,
								tool_options.gradient_type,
							)
						};
						let selected_gradient = SelectedGradient::new(gradient, layer, document).with_gradient_start(input.mouse.position);

						tool_data.selected_gradient = Some(selected_gradient);

						GradientToolFsmState::Drawing
					} else {
						GradientToolFsmState::Ready
					}
				}
			}
			(GradientToolFsmState::Drawing, GradientToolMessage::PointerMove { constrain_axis }) => {
				if let Some(selected_gradient) = &mut tool_data.selected_gradient {
					let mouse = input.mouse.position; // tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
					selected_gradient.update_gradient(mouse, responses, input.keyboard.get(constrain_axis as usize), selected_gradient.gradient.gradient_type);
				}
				GradientToolFsmState::Drawing
			}

			(GradientToolFsmState::Drawing, GradientToolMessage::PointerUp) => {
				input.mouse.finish_transaction(tool_data.drag_start, responses);
				tool_data.snap_manager.cleanup(responses);

				GradientToolFsmState::Ready
			}

			(_, GradientToolMessage::Abort) => {
				tool_data.snap_manager.cleanup(responses);
				responses.add(OverlaysMessage::Draw);

				GradientToolFsmState::Ready
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			GradientToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Gradient"),
				HintInfo::keys([Key::Shift], "Snap 15°").prepend_plus(),
			])]),
			GradientToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Snap 15°")])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}
