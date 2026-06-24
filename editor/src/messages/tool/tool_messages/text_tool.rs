#![allow(clippy::too_many_arguments)]

use super::tool_prelude::*;
use crate::consts::{COLOR_OVERLAY_BLUE_05, COLOR_OVERLAY_RED, DRAG_THRESHOLD};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::portfolio::fonts::utility_types::{FontCatalog, FontCatalogStyle};
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{
	ToolColorOptions, apply_fill_only_color_pick, apply_fill_only_enabled, refresh_slot_working_color, selection_changed_since_last_sync, solid, sync_fill_only,
};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::{Resize, viewport_zoom, window_aligned_transform};
use crate::messages::tool::common_functionality::snapping::{self, SnapCandidatePoint, SnapData};
use crate::messages::tool::common_functionality::transformation_cage::*;
use crate::messages::tool::common_functionality::utility_functions::text_bounding_box;
use crate::messages::tool::utility_types::ToolRefreshOptions;
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::choice_type::ChoiceTypeStatic;
use graphene_std::color::SRGBA8;
use graphene_std::renderer::Quad;
use graphene_std::text::{Font, TextAlign, TypesettingConfig, lines_clipping};
use graphene_std::vector::style::{Fill, FillChoice, FillChoiceUI};
use graphene_std::{Color, NodeInputDecleration};

#[derive(Default, ExtractField)]
pub struct TextTool {
	fsm_state: TextToolFsmState,
	tool_data: TextToolData,
	options: TextOptions,
}

pub struct TextOptions {
	font: Font,
	font_size: f64,
	letter_spacing: f64,
	letter_tilt: f64,
	fill: ToolColorOptions,
	align: TextAlign,
	/// Set of layers we last synced from, used to detect real selection changes vs. internal node toggles.
	last_synced_selection: Vec<LayerNodeIdentifier>,
}

impl Default for TextOptions {
	fn default() -> Self {
		Self {
			font: Font::new(graphene_std::consts::DEFAULT_FONT_FAMILY.into(), graphene_std::consts::DEFAULT_FONT_STYLE.into()),
			font_size: 24.,
			letter_spacing: 0.,
			letter_tilt: 0.,
			fill: ToolColorOptions::new_enabled(),
			align: TextAlign::default(),
			last_synced_selection: Vec::new(),
		}
	}
}

#[impl_message(Message, ToolMessage, Text)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum TextToolMessage {
	// Standard messages
	Abort,
	WorkingColorChanged,
	Overlays { context: OverlayContext },

	// Tool-specific messages
	DragStart,
	DragStop,
	EditSelected,
	BeginEditing,
	Interact,
	PointerMove { center: Key, lock_ratio: Key },
	PointerOutsideViewport { center: Key, lock_ratio: Key },
	SelectionChanged,
	TextChange { new_text: String, is_left_or_right_click: bool },
	UpdateBounds { new_text: String },
	UpdateOptions { options: TextOptionsUpdate },
	RefreshEditingFontData,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum TextOptionsUpdate {
	FillColor(FillChoice),
	FillEnabled(bool),
	Font { font: Font },
	FontSize(f64),
	Align(TextAlign),
	WorkingColorsChanged,
}

impl ToolMetadata for TextTool {
	fn icon_name(&self) -> String {
		"VectorTextTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Text Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Text
	}
}

fn create_text_widgets(tool: &TextTool, font_catalog: &FontCatalog, document: &DocumentMessageHandler) -> Vec<WidgetInstance> {
	let text_node_id = can_edit_selected(document).and_then(|layer| graph_modification_utils::get_text_id(layer, &document.network_interface));

	let apply_font = move |font: Font| -> Message {
		match text_node_id {
			Some(node_id) => {
				let resource_id = ResourceId::new();
				Message::Batched {
					messages: Box::new([
						DocumentMessage::Resource(ResourceMessage::AddFont { resource_id, font }).into(),
						NodeGraphMessage::SetInputValue {
							node_id,
							input_index: graphene_std::text::text::FontInput::INDEX,
							value: TaggedValue::Resource(resource_id),
						}
						.into(),
					]),
				}
			}
			None => TextToolMessage::UpdateOptions {
				options: TextOptionsUpdate::Font { font },
			}
			.into(),
		}
	};
	let commit_font = move |new_font: Font| -> Message {
		match text_node_id {
			Some(_) => DeferMessage::AfterGraphRun {
				messages: vec![apply_font(new_font), DocumentMessage::AddTransaction.into()],
			}
			.into(),
			None => apply_font(new_font),
		}
	};

	let font = DropdownInput::new(vec![
		font_catalog
			.iter()
			.map(|family| {
				let current_font = &tool.options.font;
				let FontCatalogStyle { weight, italic, .. } = FontCatalogStyle::from_named_style(&current_font.font_style, "");
				let new_font = Font::new(family.name.clone(), family.closest_style(weight, italic).to_named_style());
				let commit_only_font = new_font.clone();

				MenuListEntry::new(family.name.clone())
					.label(family.name.clone())
					.font(family.closest_style(400, false).preview_url(&family.name))
					.on_update(move |_| apply_font(new_font.clone()))
					.on_commit(move |_| commit_font(commit_only_font.clone()))
			})
			.collect::<Vec<_>>(),
	])
	.selected_index(font_catalog.iter().position(|family| family.name == tool.options.font.font_family).map(|i| i as u32))
	.virtual_scrolling(true)
	.widget_instance();

	let style = DropdownInput::new({
		font_catalog
			.iter()
			.find(|family| family.name == tool.options.font.font_family)
			.map(|family| {
				let build_entry = |style: &FontCatalogStyle| {
					let font_style = style.to_named_style();
					let new_font = Font::new(tool.options.font.font_family.clone(), font_style.clone());

					let new_font_for_commit = new_font.clone();

					MenuListEntry::new(font_style.clone())
						.label(font_style)
						.on_update(move |_| apply_font(new_font.clone()))
						.on_commit(move |_| commit_font(new_font_for_commit.clone()))
				};

				vec![
					family.styles.iter().filter(|style| !style.italic).map(build_entry).collect::<Vec<_>>(),
					family.styles.iter().filter(|style| style.italic).map(build_entry).collect::<Vec<_>>(),
				]
			})
			.filter(|styles| !styles.is_empty())
			.unwrap_or_default()
	})
	.selected_index(
		font_catalog
			.iter()
			.find(|family| family.name == tool.options.font.font_family)
			.and_then(|family| {
				let not_italic = family.styles.iter().filter(|style| !style.italic);
				let italic = family.styles.iter().filter(|style| style.italic);
				not_italic
					.chain(italic)
					.position(|style| Some(style) == font_catalog.find_font_style_in_catalog(&tool.options.font).as_ref())
			})
			.map(|i| i as u32),
	)
	.widget_instance();

	let size = NumberInput::new(Some(tool.options.font_size))
		.unit(" px")
		.label("Size")
		.int()
		.min(1.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| {
			TextToolMessage::UpdateOptions {
				options: TextOptionsUpdate::FontSize(number_input.value.unwrap()),
			}
			.into()
		})
		.widget_instance();
	let align_entries: Vec<_> = TextAlign::list()
		.iter()
		.flat_map(|section| section.iter())
		.map(|(item, var_meta)| {
			let align = *item;
			let entry = RadioEntryData::new(var_meta.name)
				.tooltip_label(var_meta.label)
				.tooltip_description(var_meta.description.unwrap_or_default())
				.on_update(move |_| {
					TextToolMessage::UpdateOptions {
						options: TextOptionsUpdate::Align(align),
					}
					.into()
				});
			if let Some(icon) = var_meta.icon { entry.icon(icon) } else { entry.label(var_meta.label) }
		})
		.collect();
	let align = RadioInput::new(align_entries).selected_index(Some(tool.options.align as u32)).widget_instance();
	vec![
		font,
		Separator::new(SeparatorStyle::Related).widget_instance(),
		style,
		Separator::new(SeparatorStyle::Related).widget_instance(),
		size,
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		align,
	]
}

impl ToolRefreshOptions for TextTool {
	fn refresh_options(&self, responses: &mut VecDeque<Message>) {
		// Defer to the SelectionChanged handler which has document context, required for the font/style
		// dropdowns to bind to the selected text layer's node graph inputs
		responses.add(TextToolMessage::SelectionChanged);
	}
}

impl TextTool {
	fn send_layout(&self, responses: &mut VecDeque<Message>, layout_target: LayoutTarget, font_catalog: &FontCatalog, document: &DocumentMessageHandler) {
		responses.add(LayoutMessage::SendLayout {
			layout: self.layout(font_catalog, document),
			layout_target,
		});
	}

	fn layout(&self, font_catalog: &FontCatalog, document: &DocumentMessageHandler) -> Layout {
		let mut widgets = vec![
			ColorInput::new(FillChoiceUI::from(self.options.fill.fill_choice.as_ref().unwrap_or(&FillChoice::None)))
				.mixed(self.options.fill.fill_choice.is_none())
				.narrow(true)
				.on_update(|color: &ColorInput| {
					TextToolMessage::UpdateOptions {
						options: TextOptionsUpdate::FillColor(FillChoice::from(&color.value)),
					}
					.into()
				})
				.widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		];

		widgets.extend(create_text_widgets(self, font_catalog, document));

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for TextTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		// On tool deactivation (Abort fires from the dispatcher's tool transition),
		// reset the displayed fill color so the next activation starts fresh from the current working color.
		// Guarded on `Ready` so Esc-mid-editing (which also fires Abort) doesn't wipe the user's customized fill option.
		if matches!(&message, ToolMessage::Text(TextToolMessage::Abort)) && self.fsm_state == TextToolFsmState::Ready {
			self.options.fill.fill_choice = Some(solid(context.global_tool_data.primary_color));
		}

		if context.fonts.font_catalog.is_empty() {
			responses.add_front(FontsMessage::LoadCatalog);
		}

		let options = match message {
			ToolMessage::Text(TextToolMessage::UpdateOptions { options }) => options,
			ToolMessage::Text(TextToolMessage::SelectionChanged) => {
				if let Some(layer) = can_edit_selected(context.document)
					&& let Some((_, font, typesetting)) = graph_modification_utils::get_text(layer, &context.document.network_interface, context.fonts, &context.document.resources)
				{
					self.options.align = typesetting.align;
					self.options.font_size = typesetting.font_size;
					self.options.font = font.clone();
					if let Some(editing_text) = self.tool_data.editing_text.as_mut() {
						editing_text.typesetting.align = typesetting.align;
						editing_text.typesetting.font_size = typesetting.font_size;
						editing_text.font = font;
					}
				}

				// Only sync from a text selection; reading a non-text layer's fill would pollute the swatch
				let selection_changed = selection_changed_since_last_sync(&mut self.options.last_synced_selection, context.document);
				if can_edit_selected(context.document).is_some() {
					sync_fill_only(&mut self.options.fill, true, context.global_tool_data.primary_color, context.document, selection_changed);
				} else if selection_changed {
					self.options.fill.fill_choice = Some(solid(context.global_tool_data.primary_color));
					self.options.fill.tracks_working_color = true;
				}
				// Text tool has no fill checkbox; keep enabled so new text never starts with `None`
				self.options.fill.enabled = Some(true);

				self.send_layout(responses, LayoutTarget::ToolOptions, &context.fonts.font_catalog, context.document);
				return;
			}
			_ => {
				self.fsm_state.process_event(message, &mut self.tool_data, context, &self.options, responses, true);
				return;
			}
		};
		match options {
			TextOptionsUpdate::Font { font } => {
				// The control bar font/style menus go through `SetInputValue` directly when a text layer is selected, so this
				// arm only fires when no layer is selected (control bar font is just the default for the next-created text).
				self.options.font = font.clone();
				if let Some(editing_text) = self.tool_data.editing_text.as_mut() {
					editing_text.font = font;
				}
			}
			TextOptionsUpdate::FontSize(font_size) => {
				self.options.font_size = font_size;
				if let Some(editing_text) = self.tool_data.editing_text.as_mut() {
					editing_text.typesetting.font_size = font_size;
				}
				if let Some(layer) = can_edit_selected(context.document)
					&& let Some(node_id) = graph_modification_utils::get_text_id(layer, &context.document.network_interface)
				{
					responses.add(NodeGraphMessage::SetInputValue {
						node_id,
						input_index: graphene_std::text::text::SizeInput::INDEX,
						value: TaggedValue::F64(font_size),
					});
				}
			}
			TextOptionsUpdate::Align(align) => {
				self.options.align = align;
				if let Some(editing_text) = self.tool_data.editing_text.as_mut() {
					editing_text.typesetting.align = align;
				}
				if let Some(layer) = can_edit_selected(context.document)
					&& let Some(node_id) = graph_modification_utils::get_text_id(layer, &context.document.network_interface)
				{
					responses.add(NodeGraphMessage::SetInputValue {
						node_id,
						input_index: graphene_std::text::text::AlignInput::INDEX,
						value: TaggedValue::TextAlign(align),
					});
				}
			}
			TextOptionsUpdate::FillColor(fill_choice) => {
				// Text fill is bound to the primary working color (no swap concept).
				apply_fill_only_color_pick(&mut self.options.fill, fill_choice, true, context.document, responses);
			}
			TextOptionsUpdate::FillEnabled(enabled) => {
				apply_fill_only_enabled(&mut self.options.fill, enabled, context.global_tool_data.primary_color, context.document, responses);
			}
			TextOptionsUpdate::WorkingColorsChanged => {
				refresh_slot_working_color(&mut self.options.fill, context.global_tool_data.primary_color, context.document);
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions, &context.fonts.font_catalog, context.document);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			TextToolFsmState::Ready => actions!(TextToolMessageDiscriminant;
				DragStart,
				BeginEditing,
				PointerOutsideViewport,
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
				PointerOutsideViewport,
			),
			TextToolFsmState::ResizingBounds => actions!(TextToolMessageDiscriminant;
				DragStop,
				Abort,
				PointerMove,
				PointerOutsideViewport,
			),
		}
	}
}

impl ToolTransition for TextTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			canvas_transformed: None,
			selection_changed: Some(TextToolMessage::SelectionChanged.into()),
			tool_abort: Some(TextToolMessage::Abort.into()),
			working_color_changed: Some(TextToolMessage::WorkingColorChanged.into()),
			overlay_provider: Some(|context| TextToolMessage::Overlays { context }.into()),
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
	/// The user is dragging to create a new text area.
	Placing,
	/// The user is dragging an existing text layer to move it.
	Dragging,
	/// The user is dragging to resize the text area.
	ResizingBounds,
}

#[derive(Clone, Debug)]
pub struct EditingText {
	text: String,
	font: Font,
	typesetting: TypesettingConfig,
	color: Option<Color>,
	transform: DAffine2,
}

#[derive(Clone, Debug, Copy)]
struct ResizingLayer {
	id: LayerNodeIdentifier,
	/// The transform of the text layer in document space at the start of the transformation.
	original_transform: DAffine2,
}

#[derive(Clone, Debug, Default)]
struct TextToolData {
	layer: LayerNodeIdentifier,
	editing_text: Option<EditingText>,
	new_text: String,
	drag_start: DVec2,
	drag_current: DVec2,
	resize: Resize,
	auto_panning: AutoPanning,
	// Since the overlays must be drawn without knowledge of the inputs
	cached_resize_bounds: [DVec2; 2],
	bounding_box_manager: Option<BoundingBoxManager>,
	snap_candidates: Vec<SnapCandidatePoint>,
	// TODO: Handle multiple layers in the future
	layer_dragging: Option<ResizingLayer>,
}

impl TextToolData {
	fn delete_empty_layer(&mut self, fonts: &FontsMessageHandler, responses: &mut VecDeque<Message>) -> TextToolFsmState {
		// Remove the editable textbox UI first
		self.set_editing(false, fonts, responses);

		// Delete the empty text layer and update the graph
		responses.add(NodeGraphMessage::DeleteNodes {
			node_ids: vec![self.layer.to_node()],
			delete_children: true,
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);

		TextToolFsmState::Ready
	}

	/// Set the editing state of the currently modifying layer
	fn set_editing(&self, editable: bool, fonts: &FontsMessageHandler, responses: &mut VecDeque<Message>) {
		if let Some(editing_text) = self.editing_text.as_ref().filter(|_| editable) {
			let (align, align_last) = editing_text.typesetting.align.css();
			let font_data = fonts.get_resource_or_queue_load(&editing_text.font, responses).as_ref().to_vec().into();
			responses.add(FrontendMessage::DisplayEditableTextbox {
				text: editing_text.text.clone(),
				line_height_ratio: editing_text.typesetting.line_height_ratio,
				font_size: editing_text.typesetting.font_size,
				color: editing_text.color.map_or("#000000".to_string(), |color| SRGBA8::from(color).to_css_hex()),
				font_data,
				transform: editing_text.transform.to_cols_array(),
				max_width: editing_text.typesetting.max_width,
				max_height: editing_text.typesetting.max_height,
				align: align.to_string(),
				align_last: align_last.to_string(),
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

	fn load_layer_text_node(&mut self, document: &DocumentMessageHandler, fonts: &FontsMessageHandler) -> Option<()> {
		let transform = document.metadata().transform_to_viewport(self.layer);
		let color = graph_modification_utils::get_fill_color(self.layer, &document.network_interface).unwrap_or(Color::BLACK);
		let (text, font, typesetting) = graph_modification_utils::get_text(self.layer, &document.network_interface, fonts, &document.resources)?;
		self.editing_text = Some(EditingText {
			text: text.clone(),
			font,
			typesetting,
			color: Some(color),
			transform,
		});
		self.new_text.clone_from(text);
		Some(())
	}

	fn start_editing_layer(&mut self, layer: LayerNodeIdentifier, tool_state: TextToolFsmState, document: &DocumentMessageHandler, fonts: &FontsMessageHandler, responses: &mut VecDeque<Message>) {
		if layer == LayerNodeIdentifier::ROOT_PARENT {
			log::error!("Cannot edit ROOT_PARENT in TextTooLData")
		}

		if tool_state == TextToolFsmState::Editing {
			self.set_editing(false, fonts, responses);
		}

		self.layer = layer;
		if self.load_layer_text_node(document, fonts).is_some() {
			responses.add(DocumentMessage::AddTransaction);

			self.set_editing(true, fonts, responses);

			responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![self.layer.to_node()] });
			// Make the rendered text invisible while editing
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(graph_modification_utils::get_text_id(self.layer, &document.network_interface).unwrap(), 1),
				input: NodeInput::value(TaggedValue::String("".to_string()), false),
			});
			responses.add(NodeGraphMessage::RunDocumentGraph);
		};
	}

	fn new_text(&mut self, document: &DocumentMessageHandler, editing_text: EditingText, fonts: &FontsMessageHandler, responses: &mut VecDeque<Message>) {
		self.new_text = String::new();
		responses.add(DocumentMessage::AddTransaction);

		self.layer = LayerNodeIdentifier::new_unchecked(NodeId::new());

		responses.add(FontsMessage::Load {
			font: editing_text.font.clone(),
			response: Box::new(NodeGraphMessage::RunDocumentGraph.into()),
		});
		responses.add(GraphOperationMessage::NewTextLayer {
			id: self.layer.to_node(),
			text: String::new(),
			font: editing_text.font.clone(),
			typesetting: editing_text.typesetting,
			parent: document.new_layer_parent(true),
			insert_index: 0,
		});
		responses.add(GraphOperationMessage::FillSet {
			layer: self.layer,
			fill: editing_text.color.map_or(Fill::None, Fill::Solid),
		});
		let transform = editing_text.transform;
		self.editing_text = Some(editing_text);

		self.set_editing(true, fonts, responses);

		responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![self.layer.to_node()] });

		// Defer TransformSet until after the graph has run so that downstream_transform_to_viewport
		// has correct metadata for the new layer (needed for proper placement in transformed parents).
		let layer = self.layer;
		responses.add(NodeGraphMessage::RunDocumentGraph);
		responses.add(DeferMessage::AfterGraphRun {
			messages: vec![
				GraphOperationMessage::TransformSet {
					layer,
					transform,
					transform_in: TransformIn::Viewport,
					skip_rerender: false,
				}
				.into(),
				NodeGraphMessage::RunDocumentGraph.into(),
			],
		});
	}

	fn check_click(document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, fonts: &FontsMessageHandler, responses: &mut VecDeque<Message>) -> Option<LayerNodeIdentifier> {
		let mouse = DVec2::new(input.mouse.position.x, input.mouse.position.y);
		document.metadata().all_layers().filter(|&layer| document.metadata().is_text_layer(layer)).find(|&layer| {
			let transformed_quad = document.metadata().transform_to_viewport(layer) * text_bounding_box(layer, document, fonts, responses);
			transformed_quad.contains(mouse)
		})
	}

	fn get_snap_candidates(&mut self, document: &DocumentMessageHandler, fonts: &FontsMessageHandler, responses: &mut VecDeque<Message>) {
		self.snap_candidates.clear();

		if let Some(ResizingLayer { id, .. }) = self.layer_dragging {
			let quad = document.metadata().transform_to_document(id) * text_bounding_box(id, document, fonts, responses);
			snapping::get_bbox_points(quad, &mut self.snap_candidates, snapping::BBoxSnapValues::BOUNDING_BOX, document);
		}
	}
}

fn can_edit_selected(document: &DocumentMessageHandler) -> Option<LayerNodeIdentifier> {
	let selected_nodes = document.network_interface.selected_nodes();
	let mut selected_layers = selected_nodes.selected_layers(document.metadata());
	let layer = selected_layers.next()?;

	// Check that only one layer is selected
	if selected_layers.next().is_some() {
		return None;
	}

	// Detect text layers by the presence of a Text proto node in the chain, not via `metadata().is_text_layer()` which is
	// populated lazily by the renderer after `RunDocumentGraph`. A freshly created text layer's `text_frames` entry isn't
	// available yet when SelectionChanged fires, so the metadata check would incorrectly classify it as non-text.
	graph_modification_utils::get_text_id(layer, &document.network_interface)?;

	Some(layer)
}

impl Fsm for TextToolFsmState {
	type ToolData = TextToolData;
	type ToolOptions = TextOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		transition_data: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext { document, input, fonts, viewport, .. } = transition_data;
		let fill_color = COLOR_OVERLAY_BLUE_05;

		let ToolMessage::Text(event) = event else { return self };
		match (self, event) {
			(TextToolFsmState::Editing, TextToolMessage::Overlays { context: mut overlay_context }) => {
				// While editing, the text is blanked, so the layer's rendered transform metadata is absent; read the Transform node so the overlay tracks placement
				let transform = document
					.metadata()
					.transform_to_viewport_with_first_transform_node_if_group(tool_data.layer, &document.network_interface)
					.to_cols_array();
				responses.add(FrontendMessage::DisplayEditableTextboxTransform { transform });
				if let Some(editing_text) = tool_data.editing_text.as_mut() {
					let font_resource = fonts.get_resource_or_queue_load(&editing_text.font, responses);
					let far = graphene_std::text::bounding_box(&tool_data.new_text, &font_resource, editing_text.typesetting, false);
					if far.x != 0. && far.y != 0. {
						let quad = Quad::from_box([DVec2::ZERO, far]);
						let transformed_quad = document
							.metadata()
							.transform_to_viewport_with_first_transform_node_if_group(tool_data.layer, &document.network_interface)
							* quad;
						overlay_context.quad(transformed_quad, None, Some(fill_color));
					}
				}

				TextToolFsmState::Editing
			}
			(_, TextToolMessage::Overlays { context: mut overlay_context }) => {
				if matches!(self, Self::Placing) {
					// Get the updated selection box bounds
					let quad = Quad::from_box(tool_data.cached_resize_bounds);

					// Draw a bounding box on the layers to be selected
					for layer in document.intersect_quad_no_artboards(quad, viewport) {
						overlay_context.quad(Quad::from_box(document.metadata().bounding_box_viewport(layer).unwrap_or([DVec2::ZERO; 2])), None, Some(fill_color));
					}

					overlay_context.quad(quad, None, Some(fill_color));
				}

				// TODO: implement bounding box for multiple layers
				let selected = document.network_interface.selected_nodes();
				let mut all_layers = selected.selected_visible_and_unlocked_layers(&document.network_interface);
				let layer = all_layers.find(|&layer| document.metadata().is_text_layer(layer));
				let bounds = layer.map(|layer| text_bounding_box(layer, document, fonts, responses));
				let layer_transform = layer.map(|layer| document.metadata().transform_to_viewport(layer)).unwrap_or(DAffine2::IDENTITY);

				if layer.is_none() || bounds.is_none() || layer_transform.matrix2.determinant() == 0. {
					return self;
				}

				if overlay_context.visibility_settings.transform_cage() {
					if let Some(bounds) = bounds {
						let bounding_box_manager = tool_data.bounding_box_manager.get_or_insert(BoundingBoxManager::default());
						bounding_box_manager.bounds = [bounds.0[0], bounds.0[2]];
						bounding_box_manager.transform = layer_transform;

						bounding_box_manager.render_quad(&mut overlay_context);
						// Draw red overlay if text is clipped
						let transformed_quad = layer_transform * bounds;
						if let Some((text, font, typesetting)) = graph_modification_utils::get_text(layer.unwrap(), &document.network_interface, fonts, &document.resources) {
							let font_resource = fonts.get_resource_or_queue_load(&font, responses);
							if lines_clipping(text.as_str(), &font_resource, typesetting) {
								overlay_context.line(transformed_quad.0[2], transformed_quad.0[3], Some(COLOR_OVERLAY_RED), Some(3.));
							}
						}

						bounding_box_manager.render_overlays(&mut overlay_context, false);
					}
				} else {
					tool_data.bounding_box_manager.take();
				}

				tool_data.resize.snap_manager.draw_overlays(SnapData::new(document, input, viewport), &mut overlay_context);

				self
			}
			(state, TextToolMessage::EditSelected) => {
				if let Some(layer) = can_edit_selected(document) {
					tool_data.start_editing_layer(layer, state, document, fonts, responses);
					return TextToolFsmState::Editing;
				}

				state
			}
			(TextToolFsmState::Ready, TextToolMessage::BeginEditing) => {
				if let Some(layer) = can_edit_selected(document) {
					tool_data.start_editing_layer(layer, TextToolFsmState::Ready, document, font_cache, responses);
					return TextToolFsmState::Editing;
				}

				TextToolFsmState::Ready
			}
			(TextToolFsmState::Ready, TextToolMessage::DragStart) => {
				tool_data.resize.start(document, input, viewport);
				tool_data.cached_resize_bounds = [tool_data.resize.viewport_drag_start(document); 2];
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

				let selected = document.network_interface.selected_nodes();
				let mut all_selected = selected.selected_visible_and_unlocked_layers(&document.network_interface);
				let selected = all_selected.find(|&layer| document.metadata().is_text_layer(layer));

				if dragging_bounds.is_some() {
					responses.add(DocumentMessage::StartTransaction);

					// Set the original transform
					if let Some(id) = selected {
						let original_transform = document.metadata().transform_to_document(id);
						tool_data.layer_dragging = Some(ResizingLayer { id, original_transform });
					}

					if let Some(bounds) = &mut tool_data.bounding_box_manager {
						bounds.original_bound_transform = bounds.transform;
						bounds.center_of_transformation = bounds.transform.transform_point2((bounds.bounds[0] + bounds.bounds[1]) / 2.);
					}
					tool_data.get_snap_candidates(document, fonts, responses);

					return TextToolFsmState::ResizingBounds;
				} else if let Some(clicked_layer) = TextToolData::check_click(document, input, fonts, responses) {
					responses.add(DocumentMessage::StartTransaction);

					if selected != Some(clicked_layer) {
						responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![clicked_layer.to_node()] });
					}

					let original_transform = document.metadata().transform_to_document(clicked_layer);
					tool_data.layer_dragging = Some(ResizingLayer {
						id: clicked_layer,
						original_transform,
					});
					tool_data.get_snap_candidates(document, fonts, responses);
					return TextToolFsmState::Dragging;
				}
				TextToolFsmState::Placing
			}
			(TextToolFsmState::Ready, TextToolMessage::PointerMove { .. }) => {
				// This ensures the cursor only changes if a layer is selected
				let selected = document.network_interface.selected_nodes();
				let mut all_selected = selected.selected_visible_and_unlocked_layers(&document.network_interface);
				let layer = all_selected.find(|&layer| document.metadata().is_text_layer(layer));

				let mut cursor = tool_data
					.bounding_box_manager
					.as_ref()
					.map_or(MouseCursorIcon::Text, |bounds| bounds.get_cursor(input, false, false, None));
				if layer.is_none() || cursor == MouseCursorIcon::Default {
					cursor = MouseCursorIcon::Text;
				}

				responses.add(OverlaysMessage::Draw);
				responses.add(FrontendMessage::UpdateMouseCursor { cursor });

				TextToolFsmState::Ready
			}
			(TextToolFsmState::Placing, TextToolMessage::PointerMove { center, lock_ratio }) => {
				tool_data.cached_resize_bounds = tool_data.resize.calculate_points_ignore_layer(document, input, viewport, center, lock_ratio, false);

				responses.add(OverlaysMessage::Draw);

				// Auto-panning
				let messages = [
					TextToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
					TextToolMessage::PointerMove { center, lock_ratio }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				TextToolFsmState::Placing
			}
			(TextToolFsmState::Dragging, TextToolMessage::PointerMove { center, lock_ratio }) => {
				if let Some(dragging_layer) = &tool_data.layer_dragging {
					let delta = input.mouse.position - tool_data.drag_current;
					tool_data.drag_current = input.mouse.position;

					responses.add(GraphOperationMessage::TransformChange {
						layer: dragging_layer.id,
						transform: DAffine2::from_translation(delta),
						transform_in: TransformIn::Viewport,
						skip_rerender: false,
					});

					responses.add(NodeGraphMessage::RunDocumentGraph);

					// Auto-panning
					let messages = [
						TextToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
						TextToolMessage::PointerMove { center, lock_ratio }.into(),
					];
					tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);
				}

				TextToolFsmState::Dragging
			}
			(TextToolFsmState::ResizingBounds, TextToolMessage::PointerMove { center, lock_ratio }) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager
					&& let Some(movement) = &mut bounds.selected_edges
				{
					let (centered, constrain) = (input.keyboard.key(center), input.keyboard.key(lock_ratio));
					let center_position = centered.then_some(bounds.center_of_transformation);

					let Some(dragging_layer) = tool_data.layer_dragging else { return TextToolFsmState::Ready };
					let Some(node_id) = graph_modification_utils::get_text_id(dragging_layer.id, &document.network_interface) else {
						warn!("Cannot get text node id");
						tool_data.layer_dragging.take();
						return TextToolFsmState::Ready;
					};

					let selected = vec![dragging_layer.id];
					let snap = Some(SizeSnapData {
						manager: &mut tool_data.resize.snap_manager,
						points: &mut tool_data.snap_candidates,
						snap_data: SnapData::ignore(document, input, viewport, &selected),
					});

					let (position, size) = movement.new_size(input.mouse.position, bounds.original_bound_transform, center_position, constrain, snap);
					// Normalize so the size is always positive
					let (position, size) = (position.min(position + size), size.abs());

					// Compute the offset needed for the top left in bounds space
					let original_position = movement.bounds[0].min(movement.bounds[1]);
					let translation_bounds_space = position - original_position;

					// Compute a transformation from bounds->viewport->layer
					let transform_to_layer = document.metadata().transform_to_viewport(dragging_layer.id).inverse() * bounds.original_bound_transform;
					let size_layer = transform_to_layer.transform_vector2(size);

					// Find the translation necessary from the original position in viewport space
					let translation_viewport = bounds.original_bound_transform.transform_vector2(translation_bounds_space);

					// TODO: Don't set both max_width and max_height to true at the same time, only do one based on which edge is being dragged (or both if a corner is being dragged)
					responses.add(NodeGraphMessage::SetInput {
						input_connector: InputConnector::node(node_id, graphene_std::text::text::HasMaxWidthInput::INDEX),
						input: NodeInput::value(TaggedValue::Bool(true), false),
					});
					responses.add(NodeGraphMessage::SetInput {
						input_connector: InputConnector::node(node_id, graphene_std::text::text::MaxWidthInput::INDEX),
						input: NodeInput::value(TaggedValue::F64(size_layer.x), false),
					});
					responses.add(NodeGraphMessage::SetInput {
						input_connector: InputConnector::node(node_id, graphene_std::text::text::HasMaxHeightInput::INDEX),
						input: NodeInput::value(TaggedValue::Bool(true), false),
					});
					responses.add(NodeGraphMessage::SetInput {
						input_connector: InputConnector::node(node_id, graphene_std::text::text::MaxHeightInput::INDEX),
						input: NodeInput::value(TaggedValue::F64(size_layer.y), false),
					});
					responses.add(GraphOperationMessage::TransformSet {
						layer: dragging_layer.id,
						transform: DAffine2::from_translation(translation_viewport) * document.metadata().document_to_viewport * dragging_layer.original_transform,
						transform_in: TransformIn::Viewport,
						skip_rerender: false,
					});
					responses.add(NodeGraphMessage::RunDocumentGraph);

					// Auto-panning
					let messages = [
						TextToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
						TextToolMessage::PointerMove { center, lock_ratio }.into(),
					];
					tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);
				}
				TextToolFsmState::ResizingBounds
			}
			(_, TextToolMessage::PointerMove { .. }) => {
				tool_data.resize.snap_manager.preview_draw(&SnapData::new(document, input, viewport), input.mouse.position);
				responses.add(OverlaysMessage::Draw);

				self
			}
			(TextToolFsmState::Placing, TextToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning setup
				let _ = tool_data.auto_panning.shift_viewport(input, viewport, responses);

				TextToolFsmState::Placing
			}
			(TextToolFsmState::ResizingBounds | TextToolFsmState::Dragging, TextToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, viewport, responses)
					&& let Some(bounds) = &mut tool_data.bounding_box_manager
				{
					bounds.center_of_transformation += shift;
					bounds.original_bound_transform.translation += shift;
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
				let drag_too_small = input.mouse.position.distance(tool_data.resize.viewport_drag_start(document)) < 10. * f64::EPSILON;
				let response = if drag_too_small { DocumentMessage::AbortTransaction } else { DocumentMessage::EndTransaction };
				responses.add(response);

				tool_data.resize.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				TextToolFsmState::Ready
			}
			(TextToolFsmState::Placing, TextToolMessage::DragStop) => {
				let [start, end] = tool_data.cached_resize_bounds;
				let has_dragged = (start - end).length_squared() > DRAG_THRESHOLD * DRAG_THRESHOLD;

				// Check if the user has clicked (no dragging) on some existing text
				if !has_dragged && let Some(clicked_text_layer_path) = TextToolData::check_click(document, input, fonts, responses) {
					tool_data.start_editing_layer(clicked_text_layer_path, self, document, fonts, responses);
					return TextToolFsmState::Editing;
				}

				// Otherwise create some new text. The window-aligned transform is in viewport space, so the editing overlay (a screen-space CSS matrix) carries the zoom.
				let constraint_size = has_dragged.then_some((start - end).abs() / viewport_zoom(document));
				let editing_text = EditingText {
					text: String::new(),
					transform: window_aligned_transform(document, start, DVec2::ONE),
					typesetting: TypesettingConfig {
						font_size: tool_options.font_size,
						letter_spacing: tool_options.letter_spacing,
						letter_tilt: tool_options.letter_tilt,
						max_width: constraint_size.map(|size| size.x),
						max_height: constraint_size.map(|size| size.y),
						align: tool_options.align,
						..TypesettingConfig::default()
					},
					font: Font::new(tool_options.font.font_family.clone(), tool_options.font.font_style.clone()),
					color: tool_options.fill.active_color(),
				};
				tool_data.new_text(document, editing_text, fonts, responses);
				TextToolFsmState::Editing
			}
			(TextToolFsmState::Dragging, TextToolMessage::DragStop) => {
				let drag_too_small = input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON;
				let response = if drag_too_small { DocumentMessage::AbortTransaction } else { DocumentMessage::EndTransaction };
				responses.add(response);

				tool_data.resize.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				if drag_too_small && let Some(layer_info) = &tool_data.layer_dragging {
					tool_data.start_editing_layer(layer_info.id, self, document, fonts, responses);
					return TextToolFsmState::Editing;
				}
				tool_data.layer_dragging.take();

				TextToolFsmState::Ready
			}
			(TextToolFsmState::Editing, TextToolMessage::RefreshEditingFontData) => {
				let font = Font::new(tool_options.font.font_family.clone(), tool_options.font.font_style.clone());
				let font_resource = fonts.get_resource_or_queue_load(&font, responses);
				responses.add(FrontendMessage::DisplayEditableTextboxUpdateFontData {
					font_data: font_resource.as_ref().to_vec().into(),
				});

				TextToolFsmState::Editing
			}
			(TextToolFsmState::Editing, TextToolMessage::TextChange { new_text, is_left_or_right_click }) => {
				tool_data.new_text = new_text;

				if !is_left_or_right_click {
					tool_data.set_editing(false, fonts, responses);

					responses.add(NodeGraphMessage::SetInput {
						input_connector: InputConnector::node(graph_modification_utils::get_text_id(tool_data.layer, &document.network_interface).unwrap(), 1),
						input: NodeInput::value(TaggedValue::String(tool_data.new_text.clone()), false),
					});
					responses.add(NodeGraphMessage::RunDocumentGraph);

					TextToolFsmState::Ready
				} else {
					if tool_data.new_text.is_empty() {
						return tool_data.delete_empty_layer(fonts, responses);
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
				responses.add(TextToolMessage::UpdateOptions {
					options: TextOptionsUpdate::WorkingColorsChanged,
				});
				self
			}
			(TextToolFsmState::Editing, TextToolMessage::Abort) => {
				if tool_data.new_text.is_empty() {
					return tool_data.delete_empty_layer(fonts, responses);
				}

				responses.add(FrontendMessage::TriggerTextCommit);
				TextToolFsmState::Editing
			}
			(state, TextToolMessage::Abort) => {
				if matches!(state, TextToolFsmState::ResizingBounds | TextToolFsmState::Dragging) {
					responses.add(DocumentMessage::AbortTransaction);
					if let Some(bounds) = &mut tool_data.bounding_box_manager {
						bounds.original_transforms.clear();
					}
					if matches!(state, TextToolFsmState::Dragging) {
						tool_data.layer_dragging.take();
					}
				} else {
					input.mouse.finish_transaction(tool_data.resize.viewport_drag_start(document), responses);
				}
				tool_data.resize.cleanup(responses);

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
			TextToolFsmState::Placing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")]),
			]),
			TextToolFsmState::Dragging => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
			TextToolFsmState::ResizingBounds => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Lock Aspect Ratio"), HintInfo::keys([Key::Alt], "From Center")]),
			]),
		};

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		let cursor = match self {
			TextToolFsmState::Placing => MouseCursorIcon::Crosshair,
			_ => MouseCursorIcon::Text,
		};
		responses.add(FrontendMessage::UpdateMouseCursor { cursor });
	}
}
