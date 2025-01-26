#![allow(clippy::too_many_arguments)]

use super::tool_prelude::*;
use crate::consts::DRAG_THRESHOLD;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::portfolio::document::utility_types::transformation::Selected;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, is_layer_fed_by_node_of_name};
use crate::messages::tool::common_functionality::snapping::{self, SnapCandidatePoint, SnapData};
use crate::messages::tool::common_functionality::transformation_cage::*;
use crate::messages::tool::common_functionality::{auto_panning::AutoPanning, pivot::Pivot, resize::Resize, utility_functions::text_bounding_box};

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_core::renderer::Quad;
use graphene_core::text::{load_face, Font, FontCache, TypesettingConfig};
use graphene_core::vector::style::Fill;
use graphene_core::Color;

#[derive(Default)]
pub struct TextTool {
	fsm_state: TextToolFsmState,
	tool_data: TextToolData,
	options: TextOptions,
}

pub struct TextOptions {
	font_size: f64,
	line_height_ratio: f64,
	character_spacing: f64,
	font_name: String,
	font_style: String,
	fill: ToolColorOptions,
}

impl Default for TextOptions {
	fn default() -> Self {
		Self {
			font_size: 24.,
			line_height_ratio: 1.2,
			character_spacing: 1.,
			font_name: graphene_core::consts::DEFAULT_FONT_FAMILY.into(),
			font_style: graphene_core::consts::DEFAULT_FONT_STYLE.into(),
			fill: ToolColorOptions::new_primary(),
		}
	}
}

#[impl_message(Message, ToolMessage, Text)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum TextToolMessage {
	// Standard messages
	Abort,
	WorkingColorChanged,
	Overlays(OverlayContext),

	// Tool-specific messages
	DragStart,
	DragStop,
	EditSelected,
	Interact,
	PointerMove { center: Key, lock_ratio: Key },
	PointerOutsideViewport { center: Key, lock_ratio: Key },
	TextChange { new_text: String, is_left_or_right_click: bool },
	UpdateBounds { new_text: String },
	UpdateOptions(TextOptionsUpdate),
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum TextOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	Font { family: String, style: String },
	FontSize(f64),
	LineHeightRatio(f64),
	CharacterSpacing(f64),
	WorkingColors(Option<Color>, Option<Color>),
}

impl ToolMetadata for TextTool {
	fn icon_name(&self) -> String {
		"VectorTextTool".into()
	}
	fn tooltip(&self) -> String {
		"Text Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Text
	}
}

fn create_text_widgets(tool: &TextTool) -> Vec<WidgetHolder> {
	let font = FontInput::new(&tool.options.font_name, &tool.options.font_style)
		.is_style_picker(false)
		.on_update(|font_input: &FontInput| {
			TextToolMessage::UpdateOptions(TextOptionsUpdate::Font {
				family: font_input.font_family.clone(),
				style: font_input.font_style.clone(),
			})
			.into()
		})
		.widget_holder();
	let style = FontInput::new(&tool.options.font_name, &tool.options.font_style)
		.is_style_picker(true)
		.on_update(|font_input: &FontInput| {
			TextToolMessage::UpdateOptions(TextOptionsUpdate::Font {
				family: font_input.font_family.clone(),
				style: font_input.font_style.clone(),
			})
			.into()
		})
		.widget_holder();
	let size = NumberInput::new(Some(tool.options.font_size))
		.unit(" px")
		.label("Size")
		.int()
		.min(1.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::FontSize(number_input.value.unwrap())).into())
		.widget_holder();
	let line_height_ratio = NumberInput::new(Some(tool.options.line_height_ratio))
		.label("Line Height")
		.int()
		.min(0.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.step(0.1)
		.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::LineHeightRatio(number_input.value.unwrap())).into())
		.widget_holder();
	let character_spacing = NumberInput::new(Some(tool.options.character_spacing))
		.label("Char. Spacing")
		.int()
		.min(0.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.step(0.1)
		.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::CharacterSpacing(number_input.value.unwrap())).into())
		.widget_holder();
	vec![
		font,
		Separator::new(SeparatorType::Related).widget_holder(),
		style,
		Separator::new(SeparatorType::Related).widget_holder(),
		size,
		Separator::new(SeparatorType::Related).widget_holder(),
		line_height_ratio,
		Separator::new(SeparatorType::Related).widget_holder(),
		character_spacing,
	]
}

impl LayoutHolder for TextTool {
	fn layout(&self) -> Layout {
		let mut widgets = create_text_widgets(self);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.fill.create_widgets(
			"Fill",
			true,
			|_| TextToolMessage::UpdateOptions(TextOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| TextToolMessage::UpdateOptions(TextOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorButton| TextToolMessage::UpdateOptions(TextOptionsUpdate::FillColor(color.value.as_solid())).into(),
		));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for TextTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Text(TextToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
			return;
		};
		match action {
			TextOptionsUpdate::Font { family, style } => {
				self.options.font_name = family;
				self.options.font_style = style;

				self.send_layout(responses, LayoutTarget::ToolOptions);
			}
			TextOptionsUpdate::FontSize(font_size) => self.options.font_size = font_size,
			TextOptionsUpdate::LineHeightRatio(line_height_ratio) => self.options.line_height_ratio = line_height_ratio,
			TextOptionsUpdate::CharacterSpacing(character_spacing) => self.options.character_spacing = character_spacing,
			TextOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
			}
			TextOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			TextOptionsUpdate::WorkingColors(primary, secondary) => {
				self.options.fill.primary_working_color = primary;
				self.options.fill.secondary_working_color = secondary;
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			TextToolFsmState::Ready => actions!(TextToolMessageDiscriminant;
				DragStart,
				PointerMove,
			),
			TextToolFsmState::Editing => actions!(TextToolMessageDiscriminant;
				DragStart,
				Abort,
			),
			TextToolFsmState::Placing | TextToolFsmState::Dragging => actions!(TextToolMessageDiscriminant;
				DragStop,
				Abort,
				PointerMove,
			),
			TextToolFsmState::ResizingBounds => actions!(TextToolMessageDiscriminant;
				DragStop,
				Abort,
				PointerMove,
			),
			TextToolFsmState::DraggingPivot => actions!(TextToolMessageDiscriminant;
				DragStop,
				Abort,
				PointerMove,
			),
		}
	}
}

impl ToolTransition for TextTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			canvas_transformed: None,
			tool_abort: Some(TextToolMessage::Abort.into()),
			working_color_changed: Some(TextToolMessage::WorkingColorChanged.into()),
			overlay_provider: Some(|overlay_context| TextToolMessage::Overlays(overlay_context).into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TextToolFsmState {
	/// The tool is ready to place or edit text.
	#[default]
	Ready,
	/// The user is typing in the interactive viewport text area.
	Editing,
	/// The user is clicking to add a new text layer, but hasn't dragged or released the left mouse button yet.
	Placing,
	/// The user is dragging to create a new text area.
	Dragging,
	/// The user is dragging to resize the text area.
	ResizingBounds,
	/// The user is dragging the pivot point.
	DraggingPivot,
}

#[derive(Clone, Debug)]
pub struct EditingText {
	text: String,
	font: Font,
	typesetting: TypesettingConfig,
	color: Option<Color>,
	transform: DAffine2,
}

#[derive(Clone, Debug, Default)]
struct TextToolData {
	layer: LayerNodeIdentifier,
	editing_text: Option<EditingText>,
	new_text: String,
	resize: Resize,
	auto_panning: AutoPanning,
	// Since the overlays must be drawn without knowledge of the inputs
	cached_resize_bounds: [DVec2; 2],
	bounding_box_manager: Option<BoundingBoxManager>,
	pivot: Pivot,
	snap_candidates: Vec<SnapCandidatePoint>,
	// Handle multiple layers in the future
	layer_dragging: Option<LayerNodeIdentifier>,
}

impl TextToolData {
	fn delete_empty_layer(&mut self, font_cache: &FontCache, responses: &mut VecDeque<Message>) -> TextToolFsmState {
		// Remove the editable textbox UI first
		self.set_editing(false, font_cache, responses);

		// Delete the empty text layer and update the graph
		responses.add(NodeGraphMessage::DeleteNodes {
			node_ids: vec![self.layer.to_node()],
			delete_children: true,
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);

		TextToolFsmState::Ready
	}
	/// Set the editing state of the currently modifying layer
	fn set_editing(&self, editable: bool, font_cache: &FontCache, responses: &mut VecDeque<Message>) {
		if let Some(editing_text) = self.editing_text.as_ref().filter(|_| editable) {
			responses.add(FrontendMessage::DisplayEditableTextbox {
				text: editing_text.text.clone(),
				line_height_ratio: editing_text.typesetting.line_height_ratio,
				font_size: editing_text.typesetting.font_size,
				color: editing_text.color.unwrap_or(Color::BLACK),
				url: font_cache.get_preview_url(&editing_text.font).cloned().unwrap_or_default(),
				transform: editing_text.transform.to_cols_array(),
				max_width: editing_text.typesetting.max_width,
				max_height: editing_text.typesetting.max_height,
			});
		} else {
			// Check if DisplayRemoveEditableTextbox is already in the responses queue
			let has_remove_textbox = responses.iter().any(|msg| matches!(msg, Message::Frontend(FrontendMessage::DisplayRemoveEditableTextbox)));
			responses.add(FrontendMessage::DisplayRemoveEditableTextbox);

			if has_remove_textbox {
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() });
			}
		}
	}

	fn load_layer_text_node(&mut self, document: &DocumentMessageHandler) -> Option<()> {
		let transform = document.metadata().transform_to_viewport(self.layer);
		let color = graph_modification_utils::get_fill_color(self.layer, &document.network_interface).unwrap_or(Color::BLACK);
		let (text, font, typesetting) = graph_modification_utils::get_text(self.layer, &document.network_interface)?;
		self.editing_text = Some(EditingText {
			text: text.clone(),
			font: font.clone(),
			typesetting,
			color: Some(color),
			transform,
		});
		self.new_text.clone_from(text);
		Some(())
	}

	fn start_editing_layer(&mut self, layer: LayerNodeIdentifier, tool_state: TextToolFsmState, document: &DocumentMessageHandler, font_cache: &FontCache, responses: &mut VecDeque<Message>) {
		if layer == LayerNodeIdentifier::ROOT_PARENT {
			log::error!("Cannot edit ROOT_PARENT in TextTooLData")
		}

		if tool_state == TextToolFsmState::Editing {
			self.set_editing(false, font_cache, responses);
		}

		self.layer = layer;
		if self.load_layer_text_node(document).is_some() {
			responses.add(DocumentMessage::AddTransaction);

			self.set_editing(true, font_cache, responses);

			responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![self.layer.to_node()] });
			// Make the rendered text invisible while editing
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(graph_modification_utils::get_text_id(self.layer, &document.network_interface).unwrap(), 1),
				input: NodeInput::value(TaggedValue::String("".to_string()), false),
			});
			responses.add(NodeGraphMessage::RunDocumentGraph);
		};
	}

	fn new_text(&mut self, document: &DocumentMessageHandler, editing_text: EditingText, font_cache: &FontCache, responses: &mut VecDeque<Message>) {
		// Create new text
		self.new_text = String::new();
		responses.add(DocumentMessage::AddTransaction);

		self.layer = LayerNodeIdentifier::new_unchecked(NodeId::new());

		responses.add(GraphOperationMessage::NewTextLayer {
			id: self.layer.to_node(),
			text: String::new(),
			font: editing_text.font.clone(),
			typesetting: editing_text.typesetting,
			parent: document.new_layer_parent(true),
			insert_index: 0,
		});
		responses.add(Message::StartBuffer);
		responses.add(GraphOperationMessage::FillSet {
			layer: self.layer,
			fill: if editing_text.color.is_some() { Fill::Solid(editing_text.color.unwrap()) } else { Fill::None },
		});
		responses.add(GraphOperationMessage::TransformSet {
			layer: self.layer,
			transform: editing_text.transform,
			transform_in: TransformIn::Viewport,
			skip_rerender: true,
		});
		self.editing_text = Some(editing_text);

		self.set_editing(true, font_cache, responses);

		responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![self.layer.to_node()] });

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	fn check_click(document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, font_cache: &FontCache) -> Option<LayerNodeIdentifier> {
		document
			.metadata()
			.all_layers()
			.filter(|&layer| is_layer_fed_by_node_of_name(layer, &document.network_interface, "Text"))
			.find(|&layer| {
				let transformed_quad = document.metadata().transform_to_viewport(layer) * text_bounding_box(layer, document, font_cache);
				let mouse = DVec2::new(input.mouse.position.x, input.mouse.position.y);

				transformed_quad.contains(mouse)
			})
	}

	fn get_snap_candidates(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, font_cache: &FontCache) {
		self.snap_candidates.clear();
		if let Some(layer) = self.layer_dragging {		
			if (self.snap_candidates.len() as f64) < document.snapping_state.tolerance {
				snapping::get_layer_snap_points(layer, &SnapData::new(document, input), &mut self.snap_candidates);
			}
			let quad = document.metadata().transform_to_document(layer) * text_bounding_box(layer, document, &font_cache);
			snapping::get_bbox_points(quad, &mut self.snap_candidates, snapping::BBoxSnapValues::BOUNDING_BOX, document);
		}
	}
}

fn can_edit_selected(document: &DocumentMessageHandler) -> Option<LayerNodeIdentifier> {
	let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
	let mut selected_layers = selected_nodes.selected_layers(document.metadata());
	let layer = selected_layers.next()?;

	// Check that only one layer is selected
	if selected_layers.next().is_some() {
		return None;
	}

	if !is_layer_fed_by_node_of_name(layer, &document.network_interface, "Text") {
		return None;
	}

	Some(layer)
}

impl Fsm for TextToolFsmState {
	type ToolData = TextToolData;
	type ToolOptions = TextOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, transition_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			font_cache,
			..
		} = transition_data;
		let fill_color = graphene_std::Color::from_rgb_str(crate::consts::COLOR_OVERLAY_BLUE.strip_prefix('#').unwrap())
			.unwrap()
			.with_alpha(0.05)
			.rgba_hex();

		let ToolMessage::Text(event) = event else { return self };
		match (self, event) {
			(TextToolFsmState::Editing, TextToolMessage::Overlays(mut overlay_context)) => {
				responses.add(FrontendMessage::DisplayEditableTextboxTransform {
					transform: document.metadata().transform_to_viewport(tool_data.layer).to_cols_array(),
				});
				if let Some(editing_text) = tool_data.editing_text.as_ref() {
					let buzz_face = font_cache.get(&editing_text.font).map(|data| load_face(data));
					let far = graphene_core::text::bounding_box(&tool_data.new_text, buzz_face, editing_text.typesetting);
					if far.x != 0. && far.y != 0. {
						let quad = Quad::from_box([DVec2::ZERO, far]);
						let transformed_quad = document.metadata().transform_to_viewport(tool_data.layer) * quad;
						overlay_context.quad(transformed_quad, Some(&("#".to_string() + &fill_color)));
					}
				}

				TextToolFsmState::Editing
			}
			(_, TextToolMessage::Overlays(mut overlay_context)) => {
				if matches!(self, Self::Placing | Self::Dragging) {
					// Get the updated selection box bounds
					let quad = Quad::from_box(tool_data.cached_resize_bounds);

					// Draw a bounding box on the layers to be selected
					for layer in document.intersect_quad_no_artboards(quad, input) {
						overlay_context.quad(
							Quad::from_box(document.metadata().bounding_box_viewport(layer).unwrap_or([DVec2::ZERO; 2])),
							Some(&("#".to_string() + &fill_color)),
						);
					}

					overlay_context.quad(quad, Some(&("#".to_string() + &fill_color)));
				} else {
					for layer in document
						.network_interface
						.selected_nodes(&[])
						.unwrap()
						.selected_visible_and_unlocked_layers(&document.network_interface)
					{
						if is_layer_fed_by_node_of_name(layer, &document.network_interface, "Text") {
							let transformed_quad = document.metadata().transform_to_viewport(layer) * text_bounding_box(layer, document, font_cache);
							overlay_context.quad(transformed_quad, None);
						}
					}
				}

				// TODO: implement for multiple layers
				// currently we find the first layer
				let layer = document
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_visible_and_unlocked_layers(&document.network_interface)
					.find(|layer| is_layer_fed_by_node_of_name(*layer, &document.network_interface, "Text"));
				let bounds = layer.map(|layer| text_bounding_box(layer, document, font_cache));
				let transform = layer.map(|layer| document.metadata().transform_to_viewport(layer)).unwrap_or(DAffine2::IDENTITY);

				if layer.is_none() || bounds.is_none() || transform.matrix2.determinant() == 0. {
					return self;
				}

				if let Some(bounds) = bounds {
					let bounding_box_manager = tool_data.bounding_box_manager.get_or_insert(BoundingBoxManager::default());
					bounding_box_manager.bounds = [bounds.0[0], bounds.0[2]];
					bounding_box_manager.transform = transform;
					bounding_box_manager.render_overlays(&mut overlay_context);
				} else {
					tool_data.bounding_box_manager.take();
				}

				// Update pivot
				let (min, max) = bounds.map(|quad| (quad.0[0], quad.0[2])).unwrap();
				let bounds_transform = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
				let layer_transform = document.metadata().transform_to_viewport(layer.unwrap());
				tool_data.pivot.update_box_pivot(DVec2::new(0.5, 0.5), layer_transform, bounds_transform, &mut overlay_context);

				tool_data.resize.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);

				self
			}
			(state, TextToolMessage::EditSelected) => {
				if let Some(layer) = can_edit_selected(document) {
					tool_data.start_editing_layer(layer, state, document, font_cache, responses);
					return TextToolFsmState::Editing;
				}

				state
			}
			(TextToolFsmState::Ready, TextToolMessage::DragStart) => {
				tool_data.resize.start(document, input);
				tool_data.cached_resize_bounds = [tool_data.resize.viewport_drag_start(document); 2];

				let dragging_bounds = tool_data.bounding_box_manager.as_mut().and_then(|bounding_box| {
					let edges = bounding_box.check_selected_edges(input.mouse.position);

					bounding_box.selected_edges = edges.map(|(top, bottom, left, right)| {
						let selected_edges = SelectedEdges::new(top, bottom, left, right, bounding_box.bounds);
						bounding_box.opposite_pivot = selected_edges.calculate_pivot();
						selected_edges
					});

					edges
				});

				let selected: Option<LayerNodeIdentifier> = document
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_visible_and_unlocked_layers(&document.network_interface)
					.find(|layer| is_layer_fed_by_node_of_name(*layer, &document.network_interface, "Text"));

				let state =
				// Check if the user is dragging the pivot
				if selected.is_none() {
					TextToolFsmState::Placing
				} else if tool_data.pivot.is_over(input.mouse.position) {
					// Pivot currently hardcoded to 0.5, 0.5 for text layers

					// responses.add(DocumentMessage::StartTransaction);

					// tool_data.snap_manager.start_snap(document, input, document.bounding_boxes(), true, true);
					// tool_data.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);
					TextToolFsmState::DraggingPivot
					
				// Check if the user is dragging the bounds
				} else if let Some(_selected_edges) = dragging_bounds {
					responses.add(DocumentMessage::StartTransaction);

					tool_data.layer_dragging = selected.clone();

					if let Some(bounds) = &mut tool_data.bounding_box_manager {
						bounds.original_bound_transform = bounds.transform;

						// tool_data.layer_dragging.retain(|layer| {
						// 	if *layer != LayerNodeIdentifier::ROOT_PARENT {
						// 		document.network_interface.network(&[]).unwrap().nodes.contains_key(&layer.to_node())
						// 	} else {
						// 		log::error!("ROOT_PARENT should not be part of layer_dragging");
						// 		false
						// 	}
						// });
						let selected = Vec::from([tool_data.layer_dragging.unwrap()]);

						let mut selected = Selected::new(
							&mut bounds.original_transforms,
							&mut bounds.center_of_transformation,
							&selected,
							responses,
							&document.network_interface,
							None,
							&ToolType::Text,
							None,
						);
						bounds.center_of_transformation = selected.mean_average_of_pivots();
					}
					tool_data.get_snap_candidates(document, input, &font_cache);

					TextToolFsmState::ResizingBounds
				} else {
					TextToolFsmState::Placing
				};

				return state;
			}
			(TextToolFsmState::Ready, TextToolMessage::PointerMove { .. }) => {
				// This ensures the cursor only changes if a layer is selected
				let layer = document
						.network_interface
						.selected_nodes(&[])
						.unwrap()
						.selected_visible_and_unlocked_layers(&document.network_interface)
						.find(|&layer| is_layer_fed_by_node_of_name(layer, &document.network_interface, "Text"));

				let mut cursor = MouseCursorIcon::Text;
				if layer.is_some() {
					cursor = tool_data.bounding_box_manager.as_ref().map_or(MouseCursorIcon::Text, |bounds| bounds.get_cursor(input, false));
				}
				
				// Dragging the pivot overrules the other operations
				// if tool_data.pivot.is_over(input.mouse.position) {
					// 	cursor = MouseCursorIcon::Move;
					// }
					
				responses.add(OverlaysMessage::Draw);
				responses.add(FrontendMessage::UpdateMouseCursor { cursor });

				TextToolFsmState::Ready
			}
			(Self::Placing | TextToolFsmState::Dragging, TextToolMessage::PointerMove { center, lock_ratio }) => {
				tool_data.cached_resize_bounds = tool_data.resize.calculate_points_ignore_layer(document, input, center, lock_ratio);

				responses.add(OverlaysMessage::Draw);

				// Auto-panning
				let messages = [
					TextToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
					TextToolMessage::PointerMove { center, lock_ratio }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				TextToolFsmState::Dragging
			}
			(TextToolFsmState::ResizingBounds, TextToolMessage::PointerMove { center, lock_ratio }) => {
				if let Some(ref mut bounds) = &mut tool_data.bounding_box_manager {
					if let Some(movement) = &mut bounds.selected_edges {
						let (center_bool, lock_ratio_bool) = (input.keyboard.key(center), input.keyboard.key(lock_ratio));
						let center_position = center_bool.then_some(bounds.center_of_transformation);

						let selected = Vec::from([tool_data.layer_dragging.unwrap()]);
						let snap = Some(SizeSnapData {
							manager: &mut tool_data.resize.snap_manager,
							points: &mut tool_data.snap_candidates,
							snap_data: SnapData::ignore(document, input, &selected),
						});

						// tool_data.layer_dragging.retain(|layer| {
						// 	if *layer != LayerNodeIdentifier::ROOT_PARENT {
						// 		document.network_interface.network(&[]).unwrap().nodes.contains_key(&layer.to_node())
						// 	} else {
						// 		log::error!("ROOT_PARENT should not be part of layer_dragging");
						// 		false
						// 	}
						// });

						let text_layer = tool_data.layer_dragging;
						let node_id = text_layer.map(|layer| graph_modification_utils::get_text_id(layer, &document.network_interface).unwrap());

						if node_id.is_none() || text_layer.is_none() {
							return TextToolFsmState::Ready;
						}

						let node_id = node_id.unwrap();
						let (position, size) = movement.new_size(input.mouse.position, bounds.original_bound_transform, center_position, lock_ratio_bool, snap);

						// Pivots will not work as expected here.
						// let (delta, pivot) = movement.bounds_to_scale_transform(position, size);

						// let pivot_transform = DAffine2::from_translation(pivot);
						// let _transformation = pivot_transform * delta * pivot_transform.inverse();

						// let _transformed_bounding_box = Quad::from_box([position, size].into());

						let height = (position.y - size.y).abs();
						let width = (position.x - size.x).abs();

						responses.add(NodeGraphMessage::SetInput {
							input_connector: InputConnector::node(node_id, 6),
							input: NodeInput::value(TaggedValue::OptionalF64(Some(width)), false),
						});
						responses.add(NodeGraphMessage::SetInput {
							input_connector: InputConnector::node(node_id, 7),
							input: NodeInput::value(TaggedValue::OptionalF64(Some(height)), false),
						});
						responses.add(NodeGraphMessage::RunDocumentGraph);

						// AutoPanning
						let messages = [
							TextToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
							TextToolMessage::PointerMove { center, lock_ratio }.into(),
						];
						tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);
					}
				}
				TextToolFsmState::ResizingBounds
			}
			(TextToolFsmState::DraggingPivot, TextToolMessage::PointerMove { center, lock_ratio }) => {
				// let mouse_position = input.mouse.position;
				// let snapped_mouse_position = mouse_position;
				// tool_data.pivot.set_viewport_position(snapped_mouse_position, document, responses);

				// AutoPanning
				let messages = [
					TextToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
					TextToolMessage::PointerMove { center, lock_ratio }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				TextToolFsmState::DraggingPivot
			}
			(_, TextToolMessage::PointerMove { .. }) => {
				tool_data.resize.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
				responses.add(OverlaysMessage::Draw);

				self
			}
			(TextToolFsmState::Placing | TextToolFsmState::Dragging, TextToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning setup
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				TextToolFsmState::Dragging
			}
			(TextToolFsmState::ResizingBounds, TextToolMessage::PointerOutsideViewport { .. }) => {
				// AutoPanning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, responses) {
					if let Some(ref mut bounds) = &mut tool_data.bounding_box_manager {
						bounds.center_of_transformation += shift;
						bounds.original_bound_transform.translation += shift;
					}
				}

				self
			}
			(state, TextToolMessage::PointerOutsideViewport { center, lock_ratio }) => {
				// Auto-panning stop
				let messages = [
					TextToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
					TextToolMessage::PointerMove { center, lock_ratio }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(TextToolFsmState::ResizingBounds, TextToolMessage::DragStop) => {
				let response = match input.mouse.position.distance(tool_data.resize.viewport_drag_start(document)) < 10. * f64::EPSILON {
					true => DocumentMessage::AbortTransaction,
					false => DocumentMessage::EndTransaction,
				};
				responses.add(response);

				tool_data.resize.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				TextToolFsmState::Ready
			}
			(TextToolFsmState::Placing | TextToolFsmState::Dragging, TextToolMessage::DragStop) => {
				let [start, end] = tool_data.cached_resize_bounds;
				let has_dragged = (start - end).length_squared() > DRAG_THRESHOLD * DRAG_THRESHOLD;

				// Check if the user has clicked (no dragging) on some existing text
				if !has_dragged {
					if let Some(clicked_text_layer_path) = TextToolData::check_click(document, input, font_cache) {
						tool_data.start_editing_layer(clicked_text_layer_path, self, document, font_cache, responses);
						return TextToolFsmState::Editing;
					}
				}

				// Otherwise create some new text
				let constraint_size = has_dragged.then_some((start - end).abs());
				let editing_text = EditingText {
					text: String::new(),
					transform: DAffine2::from_translation(start),
					typesetting: TypesettingConfig {
						font_size: tool_options.font_size,
						line_height_ratio: tool_options.line_height_ratio,
						max_width: constraint_size.map(|size| size.x),
						character_spacing: tool_options.character_spacing,
						max_height: constraint_size.map(|size| size.y),
					},
					font: Font::new(tool_options.font_name.clone(), tool_options.font_style.clone()),
					color: tool_options.fill.active_color(),
				};
				tool_data.new_text(document, editing_text, font_cache, responses);
				TextToolFsmState::Editing
			}
			(TextToolFsmState::DraggingPivot, TextToolMessage::DragStop) => {
				// let response = match input.mouse.position.distance(tool_data.resize.viewport_drag_start(document)) < 10. * f64::EPSILON {
				// 	true => DocumentMessage::AbortTransaction,
				// 	false => DocumentMessage::EndTransaction,
				// };
				// responses.add(response);

				// tool_data.resize.snap_manager.cleanup(responses);

				TextToolFsmState::Ready
			}
			(TextToolFsmState::Editing, TextToolMessage::TextChange { new_text, is_left_or_right_click }) => {
				tool_data.new_text = new_text;

				if !is_left_or_right_click {
					tool_data.set_editing(false, font_cache, responses);

					responses.add(NodeGraphMessage::SetInput {
						input_connector: InputConnector::node(graph_modification_utils::get_text_id(tool_data.layer, &document.network_interface).unwrap(), 1),
						input: NodeInput::value(TaggedValue::String(tool_data.new_text.clone()), false),
					});
					responses.add(NodeGraphMessage::RunDocumentGraph);

					TextToolFsmState::Ready
				} else {
					if tool_data.new_text.is_empty() {
						return tool_data.delete_empty_layer(font_cache, responses);
					}

					responses.add(FrontendMessage::TriggerTextCommit);
					TextToolFsmState::Editing
				}
			}
			(TextToolFsmState::Editing, TextToolMessage::UpdateBounds { new_text }) => {
				tool_data.new_text = new_text;
				responses.add(OverlaysMessage::Draw);
				TextToolFsmState::Editing
			}
			(_, TextToolMessage::WorkingColorChanged) => {
				responses.add(TextToolMessage::UpdateOptions(TextOptionsUpdate::WorkingColors(
					Some(global_tool_data.primary_color),
					Some(global_tool_data.secondary_color),
				)));
				self
			}
			(TextToolFsmState::Editing, TextToolMessage::Abort) => {
				if tool_data.new_text.is_empty() {
					return tool_data.delete_empty_layer(font_cache, responses);
				}

				responses.add(FrontendMessage::TriggerTextCommit);
				TextToolFsmState::Editing
			}
			(state, TextToolMessage::Abort) => {
				input.mouse.finish_transaction(tool_data.resize.viewport_drag_start(document), responses);
				tool_data.resize.cleanup(responses);

				if state == TextToolFsmState::Editing {
					tool_data.set_editing(false, font_cache, responses);
				}

				TextToolFsmState::Ready
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			TextToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Place Text")]),
				HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Place Text Box"),
					HintInfo::keys([Key::Shift], "Constrain Square").prepend_plus(),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
				]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Edit Text")]),
			]),
			TextToolFsmState::Editing => HintData(vec![HintGroup(vec![
				HintInfo::keys([Key::Control, Key::Enter], "").add_mac_keys([Key::Command, Key::Enter]),
				HintInfo::keys([Key::Escape], "Commit Changes").prepend_slash(),
			])]),
			TextToolFsmState::Placing | TextToolFsmState::Dragging => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")]),
			]),
			TextToolFsmState::ResizingBounds => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Resize Text Box")]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")]),
			]),
			TextToolFsmState::DraggingPivot => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Move Pivot")])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		let cursor = match self {
			TextToolFsmState::Dragging => MouseCursorIcon::Crosshair,
			TextToolFsmState::DraggingPivot => MouseCursorIcon::Move,
			_ => MouseCursorIcon::Text,
		};
		responses.add(FrontendMessage::UpdateMouseCursor { cursor });
	}
}
