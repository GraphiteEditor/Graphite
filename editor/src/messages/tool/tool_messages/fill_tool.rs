use super::tool_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::tool::common_functionality::color_selector::solid;
use crate::messages::tool::common_functionality::graph_modification_utils::{NodeGraphLayer, get_upstream_color_value_node_id, is_blank_paintable_layer};
use graphene_std::color::SRGBA8;
use graphene_std::raster::color::Color;
use graphene_std::vector::style::FillChoice;

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
			ColorInput::new(FillChoice::<SRGBA8>::from(&solid(self.primary_color)))
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
				// Choose the color to preview
				let use_secondary = input.keyboard.get(Key::Shift as usize);
				let preview_color = if use_secondary { global_tool_data.secondary_color } else { global_tool_data.primary_color };

				// Get the layer the user is hovering over
				if let Some(layer) = document.click(input, viewport) {
					let color_hex = SRGBA8::from(preview_color).to_css_hex();
					overlay_context.fill_path_pattern(document.metadata().layer_outline(layer), document.metadata().transform_to_viewport(layer), &color_hex);
				}

				self
			}
			(_, FillToolMessage::PointerMove | FillToolMessage::WorkingColorChanged) => {
				// Generate the hover outline
				responses.add(OverlaysMessage::Draw);
				self
			}
			(FillToolFsmState::Ready, color_event) => {
				// Blank layers and whole-expanse color chains render no clickable geometry, so fall back to a selected layer of either kind
				let clicked_layer = document.click(input, viewport).or_else(|| {
					document
						.network_interface
						.selected_nodes()
						.selected_visible_layers(&document.network_interface)
						.find(|&layer| get_upstream_color_value_node_id(layer, &document.network_interface).is_some() || is_blank_paintable_layer(layer, &document.network_interface))
				});
				let Some(layer_identifier) = clicked_layer else { return self };

				// A whole-expanse color chain (existing, or newly started on a blank layer) routes to its 'Color Value' node; geometry gets its Fill set
				let route_to_color_chain =
					get_upstream_color_value_node_id(layer_identifier, &document.network_interface).is_some() || is_blank_paintable_layer(layer_identifier, &document.network_interface);

				// If the layer is a raster layer, don't fill it, wait till the flood fill tool is implemented
				if NodeGraphLayer::is_raster_layer(layer_identifier, &mut document.network_interface) {
					return self;
				}
				let color = match color_event {
					FillToolMessage::FillPrimaryColor => global_tool_data.primary_color,
					FillToolMessage::FillSecondaryColor => global_tool_data.secondary_color,
					_ => return self,
				};

				responses.add(DocumentMessage::AddTransaction);
				if route_to_color_chain {
					responses.add(GraphOperationMessage::ColorValueSet { layer: layer_identifier, color });
				} else {
					responses.add(GraphOperationMessage::FillColorSet {
						layer: layer_identifier,
						color: Some(color),
					});
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
	use graphene_std::list::Item;
	use graphene_std::vector::fill;

	// The Fill tool writes solid colors, whose stored values the input monitor records as `Item<Color>` wires
	async fn get_fills(editor: &mut EditorTestUtils) -> Vec<Item<Color>> {
		let instrumented = match editor.eval_graph().await {
			Ok(instrumented) => instrumented,
			Err(e) => panic!("Failed to evaluate graph: {e}"),
		};

		instrumented.grab_all_input::<fill::FillInput, Item<Color>>(&editor.runtime).collect()
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
		let color = fills.first().unwrap().element();
		assert_eq!(SRGBA8::from(*color), SRGBA8::from(Color::GREEN));
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
		let color = fills.first().unwrap().element();
		assert_eq!(SRGBA8::from(*color), SRGBA8::from(Color::YELLOW));
	}

	#[tokio::test]
	async fn blank_layer_gets_whole_expanse_color() {
		use crate::messages::tool::common_functionality::graph_modification_utils::get_upstream_color_value_node_id;
		use graph_craft::document::NodeId;
		use graph_craft::document::value::TaggedValue;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor
			.handle_message(GraphOperationMessage::NewCustomLayer {
				id: NodeId::new(),
				nodes: Vec::new(),
				parent: LayerNodeIdentifier::ROOT_PARENT,
				insert_index: 0,
			})
			.await;
		let layer = editor.active_document().metadata().all_layers().next().unwrap();
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] }).await;

		editor.select_primary_color(Color::GREEN).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::empty()).await;

		let document = editor.active_document();
		let color_value_id = get_upstream_color_value_node_id(layer, &document.network_interface).expect("the fill should start a Color Value chain");
		let color_input = document
			.network_interface
			.document_network()
			.nodes
			.get(&color_value_id)
			.and_then(|node| node.input(graphene_std::math_nodes::color_value::ColorInput))
			.and_then(|input| input.as_value());
		assert!(matches!(color_input, Some(TaggedValue::Color(color)) if *color == Color::GREEN));
	}

	#[tokio::test]
	async fn node_displayed_as_layer_gets_no_color_chain() {
		use crate::messages::tool::common_functionality::graph_modification_utils::get_upstream_color_value_node_id;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// A generator node displayed as a layer looks empty from the outside, but its secondary input is a parameter of its own
		let rectangle = editor
			.create_node_by_name(DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER))
			.await;
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.set_to_node_or_layer(&rectangle, &[], true);
		let layer = LayerNodeIdentifier::new(rectangle, network_interface);
		network_interface.move_layer_to_stack(layer, LayerNodeIdentifier::ROOT_PARENT, 0, &[]);

		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![rectangle] }).await;
		assert!(editor.active_document().metadata().all_layers().any(|other| other == layer), "the node should sit in the layer stack");

		editor.select_primary_color(Color::GREEN).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::empty()).await;

		let document = editor.active_document();
		assert!(
			get_upstream_color_value_node_id(layer, &document.network_interface).is_none(),
			"only a 'Merge' layer may have a whole-expanse color chain started on it"
		);
	}
}
