use super::tool_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::tool::common_functionality::color_selector::solid;
use crate::messages::tool::common_functionality::graph_modification_utils::{self, NodeGraphLayer};
use crate::messages::tool::common_functionality::utility_functions::near_to_subpath;
use graphene_std::color::SRGBA8;
use graphene_std::raster::color::Color;
use graphene_std::vector::style::{Fill, FillChoiceUI, PathStyleType};

const STROKE_ID: DefinitionIdentifier = DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER);
const FILL_ID: DefinitionIdentifier = DefinitionIdentifier::ProtoNode(graphene_std::vector::fill::IDENTIFIER);

#[derive(Default, ExtractField)]
pub struct FillTool {
	fsm_state: FillToolFsmState,
	primary_color: Color,
}

#[impl_message(Message, ToolMessage, Fill)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum FillToolMessage {
	// Standard messages
	Abort,
	WorkingColorChanged,
	Overlays { context: OverlayContext },

	// Tool-specific messages
	PointerMove,
	PointerUp,
	FillPrimaryColor,
	FillSecondaryColor,
	SetColor { color: Option<Color> },
}

impl ToolMetadata for FillTool {
	fn icon_name(&self) -> String {
		"GeneralFillTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Fill Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Fill
	}
}

impl LayoutHolder for FillTool {
	fn layout(&self) -> Layout {
		let widgets = vec![
			ColorInput::new(FillChoiceUI::from(&solid(self.primary_color)))
				.narrow(true)
				.on_update(|color: &ColorInput| {
					FillToolMessage::SetColor {
						color: color.value.as_solid().map(Color::from),
					}
					.into()
				})
				.widget_instance(),
		];
		Layout(vec![LayoutGroup::row(widgets)])
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for FillTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		// User picked a color in the control bar: push it to the global primary working color (no tool-local customization)
		if let ToolMessage::Fill(FillToolMessage::SetColor { color: Some(color) }) = &message {
			responses.add(ToolMessage::SelectWorkingColor { color: *color, primary: true });
			return;
		}

		// Mirror the global primary working color into the control bar's color swatch
		if matches!(message, ToolMessage::Fill(FillToolMessage::WorkingColorChanged)) {
			let new_color = context.global_tool_data.primary_color;
			if self.primary_color != new_color {
				self.primary_color = new_color;
				self.send_layout(responses, LayoutTarget::ToolOptions);
			}
		}

		self.fsm_state.process_event(message, &mut (), context, &(), responses, true);
	}
	fn actions(&self) -> ActionList {
		match self.fsm_state {
			FillToolFsmState::Ready => actions!(FillToolMessageDiscriminant;
				FillPrimaryColor,
				FillSecondaryColor,
				PointerMove,
			),
			FillToolFsmState::Filling => actions!(FillToolMessageDiscriminant;
				PointerMove,
				PointerUp,
				Abort,
			),
		}
	}
}

impl ToolTransition for FillTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(FillToolMessage::Abort.into()),
			working_color_changed: Some(FillToolMessage::WorkingColorChanged.into()),
			overlay_provider: Some(|context| FillToolMessage::Overlays { context }.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FillToolFsmState {
	#[default]
	Ready,
	// Implemented as a fake dragging state that can be used to abort unwanted fills
	Filling,
}

impl Fsm for FillToolFsmState {
	type ToolData = ();
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		_tool_data: &mut Self::ToolData,
		handler_data: &mut ToolActionMessageContext,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext {
			document,
			global_tool_data,
			input,
			viewport,
			..
		} = handler_data;

		let ToolMessage::Fill(event) = event else { return self };
		match (self, event) {
			(_, FillToolMessage::Overlays { context: mut overlay_context }) => {
				if !overlay_context.visibility_settings.fillable_indicator() {
					return self;
				}
				// Choose the color to preview
				let use_secondary = input.keyboard.get(Key::Shift as usize);
				let preview_color = {
					let color = if use_secondary { global_tool_data.secondary_color } else { global_tool_data.primary_color };
					SRGBA8::from(color).to_css_hex()
				};

				// Get the layer the user is hovering
				if let Some(layer) = document.click(input, viewport)
					&& let Some(vector_data) = document.network_interface.vector_data_from_layer(layer)
				{
					let graph_layer = graph_modification_utils::NodeGraphLayer::new(layer, &document.network_interface);
					let layer_to_viewport = document.metadata().transform_to_viewport(layer);

					// Stroke
					let stroke_node = graph_layer.upstream_node_id_from_name(&STROKE_ID);
					let stroke_exists_and_visible = stroke_node.is_some_and(|stroke| document.network_interface.is_visible(&stroke, &[]));

					let mut subpaths = vector_data.stroke_bezier_paths();
					// Subpaths on a layer is considered "closed" only if all subpaths are closed.
					let is_closed_on_all = subpaths.all(|subpath| subpath.closed);
					subpaths = vector_data.stroke_bezier_paths();
					let near_to_stroke = subpaths.any(|subpath| {
						near_to_subpath(
							input.mouse.position,
							subpath,
							is_closed_on_all,
							vector_data.style.stroke(),
							layer_to_viewport,
							Some(overlay_context.clone()),
						)
					});

					// Fill
					let fill_node = graph_layer.upstream_node_id_from_name(&FILL_ID);
					let fill_exists_and_visible = fill_node.is_some_and(|fill| document.network_interface.is_visible(&fill, &[]));

					if stroke_exists_and_visible && near_to_stroke {
						overlay_context.preview_fill(PathStyleType::Stroke, &vector_data, preview_color.as_str(), layer_to_viewport, is_closed_on_all);
					} else if fill_exists_and_visible {
						overlay_context.preview_fill(PathStyleType::Fill, &vector_data, preview_color.as_str(), layer_to_viewport, is_closed_on_all);
					}
				}
				self
			}
			(_, FillToolMessage::PointerMove | FillToolMessage::WorkingColorChanged) => {
				// Generate the hover outline
				responses.add(OverlaysMessage::Draw);
				self
			}
			(FillToolFsmState::Ready, color_event) => {
				let Some(layer) = document.click(input, viewport) else {
					return self;
				};
				// If the layer is a raster layer, don't fill it, wait till the flood fill tool is implemented
				if NodeGraphLayer::is_raster_layer(layer, &mut document.network_interface) {
					return self;
				}
				let fill = match color_event {
					FillToolMessage::FillPrimaryColor => Fill::Solid(global_tool_data.primary_color),
					FillToolMessage::FillSecondaryColor => Fill::Solid(global_tool_data.secondary_color),
					_ => return self,
				};
				let stroke_color = match color_event {
					FillToolMessage::FillPrimaryColor => global_tool_data.primary_color,
					FillToolMessage::FillSecondaryColor => global_tool_data.secondary_color,
					_ => return self,
				};

				responses.add(DocumentMessage::AddTransaction);

				if let Some(vector_data) = document.network_interface.vector_data_from_layer(layer) {
					let graph_layer = graph_modification_utils::NodeGraphLayer::new(layer, &document.network_interface);
					let layer_to_viewport = document.metadata().transform_to_viewport(layer);

					// Stroke
					let stroke_node = graph_layer.upstream_node_id_from_name(&STROKE_ID);
					let stroke_exists_and_visible = stroke_node.is_some_and(|stroke| document.network_interface.is_visible(&stroke, &[]));
					let stroke = vector_data.style.stroke();

					let mut subpaths = vector_data.stroke_bezier_paths();
					// Subpaths on a layer is considered "closed" only if all subpaths are closed.
					let is_closed_on_all = subpaths.all(|subpath| subpath.closed);
					subpaths = vector_data.stroke_bezier_paths();
					let near_to_stroke = subpaths.any(|subpath| near_to_subpath(input.mouse.position, subpath, is_closed_on_all, stroke.clone(), layer_to_viewport, None));

					// Fill
					let fill_node = graph_layer.upstream_node_id_from_name(&FILL_ID);
					let fill_exists_and_visible = fill_node.is_some_and(|fill| document.network_interface.is_visible(&fill, &[]));

					if stroke_exists_and_visible && near_to_stroke {
						responses.add(GraphOperationMessage::StrokeColorSet { layer, stroke_color });
					} else if fill_exists_and_visible {
						responses.add(GraphOperationMessage::FillSet { layer, fill });
					}
				}

				FillToolFsmState::Filling
			}
			(FillToolFsmState::Filling, FillToolMessage::PointerUp) => FillToolFsmState::Ready,
			(FillToolFsmState::Filling, FillToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);

				FillToolFsmState::Ready
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			FillToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::Lmb, "Fill with Primary"),
				HintInfo::keys([Key::Shift], "Fill with Secondary").prepend_plus(),
			])]),
			FillToolFsmState::Filling => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
		};

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

#[cfg(test)]
mod test_fill {
	pub use crate::test_utils::test_prelude::*;
	use graphene_std::color::SRGBA8;
	use graphene_std::vector::fill;
	use graphene_std::vector::style::Fill;

	async fn get_fills(editor: &mut EditorTestUtils) -> Vec<Fill> {
		let instrumented = match editor.eval_graph().await {
			Ok(instrumented) => instrumented,
			Err(e) => panic!("Failed to evaluate graph: {e}"),
		};

		instrumented.grab_all_input::<fill::FillInput<Fill>>(&editor.runtime).collect()
	}

	#[tokio::test]
	async fn ignore_artboard() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::empty()).await;
		assert!(get_fills(&mut editor,).await.is_empty());
	}

	#[tokio::test]
	async fn ignore_raster() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.create_raster_image(Image::new(100, 100, Color::WHITE), Some((0., 0.))).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::empty()).await;
		assert!(get_fills(&mut editor,).await.is_empty());
	}

	#[tokio::test]
	async fn primary() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::empty()).await;
		let fills = get_fills(&mut editor).await;
		assert_eq!(fills.len(), 1);
		assert_eq!(SRGBA8::from(fills[0].as_solid().unwrap()), SRGBA8::from(Color::GREEN));
	}

	#[tokio::test]
	async fn secondary() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.select_secondary_color(Color::YELLOW).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::SHIFT).await;
		let fills = get_fills(&mut editor).await;
		assert_eq!(fills.len(), 1);
		assert_eq!(SRGBA8::from(fills[0].as_solid().unwrap()), SRGBA8::from(Color::YELLOW));
	}
}
