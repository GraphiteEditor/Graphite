use super::tool_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::tool::common_functionality::graph_modification_utils::{self, NodeGraphLayer, get_stroke_width};
use graph_craft::document::value::TaggedValue;
use graphene_core::vector::style::Fill;
use graphene_std::vector::PointId;
use graphene_std::vector::style::Stroke;

#[derive(Default)]
pub struct FillTool {
	fsm_state: FillToolFsmState,
}

#[impl_message(Message, ToolMessage, Fill)]
#[derive(PartialEq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FillToolMessage {
	// Standard messages
	Abort,
	WorkingColorChanged,
	Overlays(OverlayContext),

	// Tool-specific messages
	PointerMove,
	PointerUp,
	FillPrimaryColor,
	FillSecondaryColor,
}

impl ToolMetadata for FillTool {
	fn icon_name(&self) -> String {
		"GeneralFillTool".into()
	}
	fn tooltip(&self) -> String {
		"Fill Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Fill
	}
}

impl LayoutHolder for FillTool {
	fn layout(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::default())
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for FillTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		self.fsm_state.process_event(message, &mut (), tool_data, &(), responses, true);
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
			overlay_provider: Some(|overlay_context| FillToolMessage::Overlays(overlay_context).into()),
			..Default::default()
		}
	}
}

pub fn close_to_subpath(mouse_pos: DVec2, subpath: bezier_rs::Subpath<PointId>, stroke_width: f64, layer_to_viewport_transform: DAffine2) -> bool {
	let mouse_pos = layer_to_viewport_transform.inverse().transform_point2(mouse_pos);
	let max_stroke_distance = stroke_width;

	if let Some((segment_index, t)) = subpath.project(mouse_pos) {
		let nearest_point = subpath.evaluate(bezier_rs::SubpathTValue::Parametric { segment_index, t });
		// debug!("max_stroke_distance: {max_stroke_distance}");
		// debug!("mouse-stroke distance: {:?}", (mouse_pos - nearest_point).length());
		(mouse_pos - nearest_point).length_squared() <= max_stroke_distance
	} else {
		false
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

	fn transition(self, event: ToolMessage, _tool_data: &mut Self::ToolData, handler_data: &mut ToolActionHandlerData, _tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document, global_tool_data, input, ..
		} = handler_data;

		let ToolMessage::Fill(event) = event else { return self };
		match (self, event) {
			(_, FillToolMessage::Overlays(mut overlay_context)) => {
				// Choose the working color to preview
				let use_secondary = input.keyboard.get(Key::Shift as usize);
				let preview_color = if use_secondary { global_tool_data.secondary_color } else { global_tool_data.primary_color };

				if !overlay_context.visibility_settings.fillable_indicator() {
					return self;
				};
				// Get the layer the user is hovering over
				if let Some(layer) = document.click(input) {
					if let Some(vector_data) = document.network_interface.compute_modified_vector(layer) {
						let mut subpaths = vector_data.stroke_bezier_paths();
						let graph_layer = graph_modification_utils::NodeGraphLayer::new(layer, &document.network_interface);

						// Stroke
						let stroke_node = graph_layer.upstream_node_id_from_name("Stroke");
						let stroke_width = get_stroke_width(layer, &document.network_interface).unwrap_or(1.0);
						let zoom = document.document_ptz.zoom();
						let modified_stroke_width = stroke_width * zoom;
						let stroke_exists_and_visible = stroke_node.is_some_and(|stroke| document.network_interface.is_visible(&stroke, &[]));
						let close_to_stroke = subpaths.any(|subpath| close_to_subpath(input.mouse.position, subpath, stroke_width, document.metadata().transform_to_viewport(layer)));

						// Fill
						let fill_node = graph_layer.upstream_node_id_from_name("Fill");
						let fill_exists_and_visible = fill_node.is_some_and(|fill| document.network_interface.is_visible(&fill, &[]));

						if stroke_exists_and_visible && close_to_stroke {
							let overlay_stroke = || {
								let mut stroke = Stroke::new(Some(preview_color), modified_stroke_width);
								stroke.transform = document.metadata().transform_to_viewport(layer);
								let line_cap = graph_layer.find_input("Stroke", 5).unwrap();
								stroke.line_cap = if let TaggedValue::LineCap(line_cap) = line_cap { *line_cap } else { return None };
								let line_join = graph_layer.find_input("Stroke", 6).unwrap();
								stroke.line_join = if let TaggedValue::LineJoin(line_join) = line_join { *line_join } else { return None };
								let miter_limit = graph_layer.find_input("Stroke", 7).unwrap();
								stroke.line_join_miter_limit = if let TaggedValue::F64(miter_limit) = miter_limit { *miter_limit } else { return None };

								Some(stroke)
							};

							if let Some(stroke) = overlay_stroke() {
								subpaths = vector_data.stroke_bezier_paths();
								overlay_context.fill_stroke(subpaths, &stroke);
							}
						} else if fill_exists_and_visible {
							subpaths = vector_data.stroke_bezier_paths();
							overlay_context.fill_path(subpaths, document.metadata().transform_to_viewport(layer), &preview_color, true, stroke_exists_and_visible, Some(modified_stroke_width));
						}
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
				// Get the layer the user is hovering over
				let Some(layer) = document.click(input) else {
					return self;
				};
				// If the layer is a raster layer, don't fill it, wait till the flood fill tool is implemented
				if NodeGraphLayer::is_raster_layer(layer, &mut document.network_interface) {
					return self;
				}
				let fill = match color_event {
					FillToolMessage::FillPrimaryColor => Fill::Solid(global_tool_data.primary_color.to_gamma_srgb()),
					FillToolMessage::FillSecondaryColor => Fill::Solid(global_tool_data.secondary_color.to_gamma_srgb()),
					_ => return self,
				};
				let stroke_color = match color_event {
					FillToolMessage::FillPrimaryColor => global_tool_data.primary_color.to_gamma_srgb(),
					FillToolMessage::FillSecondaryColor => global_tool_data.secondary_color.to_gamma_srgb(),
					_ => return self,
				};

				responses.add(DocumentMessage::AddTransaction);
				if let Some(vector_data) = document.network_interface.compute_modified_vector(layer) {
					let mut subpaths = vector_data.stroke_bezier_paths();
					let graph_layer = graph_modification_utils::NodeGraphLayer::new(layer, &document.network_interface);

					// Stroke
					let stroke_node = graph_layer.upstream_node_id_from_name("Stroke");
					let stroke_width = get_stroke_width(layer, &document.network_interface).unwrap_or(1.0);
					let stroke_exists_and_visible = stroke_node.is_some_and(|stroke| document.network_interface.is_visible(&stroke, &[]));
					let close_to_stroke = subpaths.any(|subpath| close_to_subpath(input.mouse.position, subpath, stroke_width, document.metadata().transform_to_viewport(layer)));

					// Fill
					let fill_node = graph_layer.upstream_node_id_from_name("Fill");
					let fill_exists_and_visible = fill_node.is_some_and(|fill| document.network_interface.is_visible(&fill, &[]));

					if stroke_exists_and_visible && close_to_stroke {
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

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

#[cfg(test)]
mod test_fill {
	pub use crate::test_utils::test_prelude::*;
	use graphene_core::vector::fill;
	use graphene_std::vector::style::Fill;

	async fn get_fills(editor: &mut EditorTestUtils) -> Vec<Fill> {
		let instrumented = editor.eval_graph().await;

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
		assert_eq!(fills[0].as_solid().unwrap().to_rgba8_srgb(), Color::GREEN.to_rgba8_srgb());
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
		assert_eq!(fills[0].as_solid().unwrap().to_rgba8_srgb(), Color::YELLOW.to_rgba8_srgb());
	}
}
