use super::tool_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface};
use crate::messages::tool::common_functionality::color_selector::solid;
use crate::messages::tool::common_functionality::graph_modification_utils::{NodeGraphLayer, get_upstream_color_value_node_id, gradient_chain_target_input, replaceable_paint_chain};
use graphene_std::color::SRGBA8;
use graphene_std::raster::color::Color;
use graphene_std::vector::misc::dvec2_to_point;
use graphene_std::vector::style::FillChoice;
use kurbo::{BezPath, DEFAULT_ACCURACY, Rect, Shape};

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

				// Pattern the layer the fill would land on, over its whole expanse when the color is its entire content
				if let Some(layer) = fill_target_layer(document, input, viewport) {
					let color_hex = SRGBA8::from(preview_color).to_css_hex();

					if paints_whole_expanse(layer, &document.network_interface) {
						let expanse = whole_expanse_rect(layer, document, overlay_context.viewport.size().into_dvec2());
						overlay_context.fill_path_pattern(&expanse, DAffine2::IDENTITY, &color_hex);
					} else {
						let mut outline = BezPath::new();
						for path in document.metadata().layer_outline(layer) {
							outline.extend(path.elements().iter().copied());
						}
						overlay_context.fill_path_pattern(&outline, document.metadata().transform_to_viewport(layer), &color_hex);
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
				let Some(layer_identifier) = fill_target_layer(document, input, viewport) else { return self };

				// A whole-expanse color chain (existing, or newly started on a blank layer) routes to its 'Color Value' node; geometry gets its Fill set
				let route_to_color_chain = routes_to_color_chain(layer_identifier, &document.network_interface);

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

/// Whether the fill lands on the layer's 'Color Value' node rather than on geometry's Fill, which includes taking over
/// a layer the Gradient tool painted, since a whole-expanse gradient gives way to a solid color.
fn routes_to_color_chain(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> bool {
	get_upstream_color_value_node_id(layer, network_interface).is_some() || replaceable_paint_chain(layer, network_interface).is_some()
}

/// The layer the fill acts on: the one under the cursor, or else a selected layer painted through a color chain,
/// since blank layers and whole-expanse colors render no clickable geometry.
fn fill_target_layer(document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler) -> Option<LayerNodeIdentifier> {
	document.click(input, viewport).or_else(|| {
		document
			.network_interface
			.selected_nodes()
			.selected_visible_layers(&document.network_interface)
			.find(|&layer| routes_to_color_chain(layer, &document.network_interface))
	})
}

/// Whether the color is the layer's whole content rather than paint applied to its geometry, meaning there is no
/// outline to pattern and the preview covers the layer's whole expanse instead.
fn paints_whole_expanse(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> bool {
	gradient_chain_target_input(layer, network_interface) == InputConnector::layer_secondary_input(layer.to_node()) && routes_to_color_chain(layer, network_interface)
}

/// The viewport-space area a whole-expanse color paints: the artboard containing the layer, since the color fills it,
/// or else the visible viewport for a layer living outside any artboard.
fn whole_expanse_rect(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, viewport_size: DVec2) -> BezPath {
	let containing_artboard = layer
		.ancestors(document.metadata())
		.find(|&ancestor| ancestor != LayerNodeIdentifier::ROOT_PARENT && document.network_interface.is_artboard(&ancestor.to_node(), &[]));

	let [min, max] = containing_artboard
		.and_then(|artboard| document.metadata().bounding_box_viewport(artboard))
		.unwrap_or([DVec2::ZERO, viewport_size]);

	Rect::from_points(dvec2_to_point(min), dvec2_to_point(max)).to_path(DEFAULT_ACCURACY)
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

		instrumented.grab_all_input::<fill::PaintInput, Item<Color>>(&editor.runtime).collect()
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
	async fn replaces_a_whole_expanse_gradient_with_a_solid_color() {
		use crate::messages::tool::common_functionality::graph_modification_utils::{get_upstream_color_value_node_id, get_upstream_gradient_value_node_id};
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

		// Paint the layer's whole expanse with a gradient, which the Fill tool then takes over
		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;
		assert!(get_upstream_gradient_value_node_id(layer, &editor.active_document().network_interface).is_some());

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

		// The replaced gradient nodes are gone from the graph rather than left orphaned
		let gradient_reference = DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::gradient_value::IDENTIFIER);
		let leftover_gradient_nodes = document
			.network_interface
			.document_network()
			.nodes
			.keys()
			.filter(|node_id| document.network_interface.reference(node_id, &[]).as_ref() == Some(&gradient_reference))
			.count();
		assert_eq!(leftover_gradient_nodes, 0, "the gradient it replaced should be deleted");
	}

	#[tokio::test]
	async fn replacing_a_shared_gradient_leaves_it_for_its_other_layer() {
		use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, OutputConnector};
		use crate::messages::tool::common_functionality::graph_modification_utils::{get_upstream_color_value_node_id, get_upstream_gradient_value_node_id};
		use graph_craft::document::NodeId;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let painted_id = NodeId::new();
		let sharing_id = NodeId::new();
		for id in [painted_id, sharing_id] {
			editor
				.handle_message(GraphOperationMessage::NewCustomLayer {
					id,
					nodes: Vec::new(),
					parent: LayerNodeIdentifier::ROOT_PARENT,
					insert_index: 0,
				})
				.await;
		}
		let (painted, sharing) = (LayerNodeIdentifier::new_unchecked(painted_id), LayerNodeIdentifier::new_unchecked(sharing_id));

		// Give one layer a whole-expanse gradient, then wire that same 'Gradient Value' node into the other layer
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![painted_id] }).await;
		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;
		let gradient_value_id = get_upstream_gradient_value_node_id(painted, &editor.active_document().network_interface).expect("the drag should start a gradient chain");
		editor
			.active_document_mut()
			.network_interface
			.create_wire(&OutputConnector::primary_output(gradient_value_id), &InputConnector::layer_secondary_input(sharing_id), &[]);

		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![painted_id] }).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::empty()).await;

		let document = editor.active_document();
		assert!(
			get_upstream_color_value_node_id(painted, &document.network_interface).is_some(),
			"the filled layer should get its own Color Value chain"
		);
		assert!(
			document.network_interface.document_network().nodes.contains_key(&gradient_value_id),
			"a gradient the rest of the graph still draws from must not be deleted"
		);
		assert_eq!(
			get_upstream_gradient_value_node_id(sharing, &document.network_interface),
			Some(gradient_value_id),
			"the other layer should keep its gradient"
		);
	}

	#[tokio::test]
	async fn replacing_a_gradient_shared_through_a_transform_leaves_the_other_layer_alone() {
		use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, OutputConnector};
		use crate::messages::tool::common_functionality::graph_modification_utils::{get_fill_node_id_with_direct_fill_input, get_upstream_color_value_node_id, get_upstream_gradient_value_node_id};
		use graph_craft::document::NodeId;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// An ellipse layer whose Fill is painted by the same chain that fills the blank layer's whole expanse
		editor.drag_tool(ToolType::Ellipse, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let ellipse = editor.active_document().metadata().all_layers().next().unwrap();

		let painted_id = NodeId::new();
		editor
			.handle_message(GraphOperationMessage::NewCustomLayer {
				id: painted_id,
				nodes: Vec::new(),
				parent: LayerNodeIdentifier::ROOT_PARENT,
				insert_index: 0,
			})
			.await;
		let painted = LayerNodeIdentifier::new_unchecked(painted_id);
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![painted_id] }).await;

		// Nudging leaves a 'Transform' node, and the gradient drag paints through it
		editor
			.handle_message(DocumentMessage::NudgeSelectedLayers {
				delta_x: 10.,
				delta_y: 0.,
				resize: Key::Shift,
				resize_opposite: Key::Alt,
			})
			.await;
		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;
		let gradient_value_id = get_upstream_gradient_value_node_id(painted, &editor.active_document().network_interface).expect("the drag should start a gradient chain");
		let shared_transform_id = editor
			.active_document()
			.network_interface
			.upstream_output_connector(&InputConnector::layer_secondary_input(painted_id), &[])
			.and_then(|output| output.node_id())
			.expect("the nudge should leave a Transform node feeding the layer");

		// Branch that same Transform into the ellipse's Fill, so both layers draw from it
		let ellipse_fill_id = get_fill_node_id_with_direct_fill_input(ellipse, &editor.active_document().network_interface).expect("the ellipse should have a Fill node");
		editor.active_document_mut().network_interface.create_wire(
			&OutputConnector::primary_output(shared_transform_id),
			&InputConnector::node(ellipse_fill_id, graphene_std::vector::fill::PaintInput),
			&[],
		);

		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![painted_id] }).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::empty()).await;

		let document = editor.active_document();
		assert!(
			get_upstream_color_value_node_id(painted, &document.network_interface).is_some(),
			"the filled layer should get its own Color Value chain"
		);
		assert!(
			document.network_interface.document_network().nodes.contains_key(&gradient_value_id),
			"the gradient still painting the ellipse must not be deleted"
		);
		assert_eq!(
			document
				.network_interface
				.upstream_output_connector(&InputConnector::node(ellipse_fill_id, graphene_std::vector::fill::PaintInput), &[])
				.and_then(|output| output.node_id()),
			Some(shared_transform_id),
			"the ellipse should keep being painted by the shared chain"
		);
		assert_eq!(
			get_upstream_gradient_value_node_id(ellipse, &document.network_interface),
			Some(gradient_value_id),
			"the ellipse's fill should still resolve to the gradient"
		);
	}

	#[tokio::test]
	async fn nudged_blank_layer_keeps_its_transform() {
		use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
		use crate::messages::tool::common_functionality::graph_modification_utils::get_upstream_color_value_node_id;
		use graph_craft::document::NodeId;

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

		// Nudging an empty layer leaves a 'Transform' node in its chain, even though the layer still paints nothing
		editor
			.handle_message(DocumentMessage::NudgeSelectedLayers {
				delta_x: 10.,
				delta_y: 0.,
				resize: Key::Shift,
				resize_opposite: Key::Alt,
			})
			.await;
		let layer_content_input = InputConnector::layer_secondary_input(layer.to_node());
		let transform_id = editor
			.active_document()
			.network_interface
			.upstream_output_connector(&layer_content_input, &[])
			.and_then(|output| output.node_id())
			.expect("the nudge should leave a Transform node feeding the layer");

		editor.select_primary_color(Color::GREEN).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::empty()).await;

		let network_interface = &editor.active_document().network_interface;
		let color_value_id = get_upstream_color_value_node_id(layer, network_interface).expect("the fill should start a Color Value chain");
		assert_eq!(
			network_interface.upstream_output_connector(&layer_content_input, &[]).and_then(|output| output.node_id()),
			Some(transform_id),
			"the Transform node should still feed the layer"
		);
		assert_eq!(
			network_interface
				.upstream_output_connector(&InputConnector::primary_input(transform_id), &[])
				.and_then(|output| output.node_id()),
			Some(color_value_id),
			"the color should be painted through the preserved Transform node"
		);
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
