#![allow(clippy::too_many_arguments)]
#[cfg(target_family = "wasm")]
use crate::editor_wrapper::EditorWrapper;
use graphite_proc_macros::editor_commands;
use serde::{Deserialize, Serialize};
use tsify::Tsify;
#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

editor_commands! {
	use crate::helpers::translate_key;
	use editor::messages::clipboard::utility_types::ClipboardContentRaw;
	use editor::messages::input_mapper::utility_types::input_keyboard::ModifierKeys;
	use editor::messages::input_mapper::utility_types::input_mouse::{EditorMouseState, ScrollDelta};
	use editor::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
	use editor::messages::portfolio::document::utility_types::network_interface::ImportOrExport;
	use editor::messages::portfolio::utility_types::PanelGroupId;
	use editor::messages::prelude::*;
	use editor::messages::tool::tool_messages::tool_prelude::WidgetId;
	use graph_craft::document::NodeId;
	use graphene_std::raster::color::Color;
	use graphene_std::vector::style::FillChoice;
	use std::path::PathBuf;

	/// Re-sends all UI layouts to the frontend. Called during HMR re-mounts when the frontend has lost its layout state.
	fn resend_all_layouts() -> Message {
		LayoutMessage::ResendAllLayouts.into()
	}

	/// First message of a session, sent once the frontend is ready
	fn init_portfolio() -> Message {
		PortfolioMessage::Init.into()
	}

	/// Per-frame tick: advances the animation clock and broadcasts the animation frame event
	fn animation_frame(timestamp: u64) -> Message {
		Message::Batched {
			messages: Box::new([
				InputPreprocessorMessage::CurrentTime { timestamp }.into(),
				AnimationMessage::IncrementFrameCounter.into(),
				// Used by auto-panning, but this could possibly be refactored in the future, see:
				// <https://github.com/GraphiteEditor/Graphite/pull/2562#discussion_r2041102786>
				BroadcastMessage::TriggerEvent(EventMessage::AnimationFrame).into(),
			]),
		}
	}

	fn auto_save_all_documents() -> Message {
		PortfolioMessage::AutoSaveAllDocuments.into()
	}

	fn add_primary_import() -> Message {
		Message::Batched {
			messages: Box::new([DocumentMessage::AddTransaction.into(), NodeGraphMessage::AddPrimaryImport.into()]),
		}
	}

	fn add_secondary_import() -> Message {
		Message::Batched {
			messages: Box::new([DocumentMessage::AddTransaction.into(), NodeGraphMessage::AddSecondaryImport.into()]),
		}
	}

	fn add_primary_export() -> Message {
		Message::Batched {
			messages: Box::new([DocumentMessage::AddTransaction.into(), NodeGraphMessage::AddPrimaryExport.into()]),
		}
	}

	fn add_secondary_export() -> Message {
		Message::Batched {
			messages: Box::new([DocumentMessage::AddTransaction.into(), NodeGraphMessage::AddSecondaryExport.into()]),
		}
	}

	/// Start Pointer Lock
	fn app_window_pointer_lock() -> Message {
		AppWindowMessage::PointerLock.into()
	}

	/// Minimizes the application window to the taskbar or dock
	fn app_window_minimize() -> Message {
		AppWindowMessage::Minimize.into()
	}

	/// Toggles minimizing or restoring down the application window
	fn app_window_maximize() -> Message {
		AppWindowMessage::Maximize.into()
	}

	fn app_window_fullscreen() -> Message {
		AppWindowMessage::Fullscreen.into()
	}

	/// Closes the application window
	fn app_window_close() -> Message {
		AppWindowMessage::Close.into()
	}

	/// Drag the application window
	fn app_window_drag() -> Message {
		AppWindowMessage::Drag.into()
	}

	/// Displays a dialog with an error message
	fn error_dialog(title: String, description: String) -> Message {
		DialogMessage::DisplayDialogError { title, description }.into()
	}

	/// Update the value of a given UI widget, but don't commit it to the history (unless `commit_layout()` is called, which handles that)
	fn widget_value_update(layout_target: LayoutTarget, widget_id: u64, value: Any, resend_widget: bool) -> Message {
		let widget_id = WidgetId(widget_id);
		let update = LayoutMessage::WidgetValueUpdate {
			layout_target,
			widget_id,
			value: value.cast(),
		};
		if resend_widget {
			Message::Batched {
				messages: Box::new([update.into(), LayoutMessage::ResendActiveWidget { layout_target, widget_id }.into()]),
			}
		} else {
			update.into()
		}
	}

	/// Commit the value of a given UI widget to the history
	fn widget_value_commit(layout_target: LayoutTarget, widget_id: u64, value: Any) -> Message {
		LayoutMessage::WidgetValueCommit {
			layout_target,
			widget_id: WidgetId(widget_id),
			value: value.cast(),
		}
		.into()
	}

	/// Update the value of a given UI widget, and commit it to the history
	fn widget_value_commit_and_update(layout_target: LayoutTarget, widget_id: u64, value: Any, resend_widget: bool) -> Message {
		let widget_id = WidgetId(widget_id);
		let mut messages: Vec<Message> = vec![
			LayoutMessage::WidgetValueCommit {
				layout_target,
				widget_id,
				value: value.cast(),
			}
			.into(),
			LayoutMessage::WidgetValueUpdate {
				layout_target,
				widget_id,
				value: value.cast(),
			}
			.into(),
		];
		if resend_widget {
			messages.push(LayoutMessage::ResendActiveWidget { layout_target, widget_id }.into());
		}
		// Close out a transaction that the widget's `on_commit` opened (if any), so a single click on widgets like the
		// NumberInput's increment buttons collapses into one history step instead of leaving the transaction in `Modified`
		messages.push(DocumentMessage::EndTransaction.into());
		Message::Batched { messages: messages.into() }
	}

	/// Fire a widget's drag-drop action (e.g. when a draggable item is dropped on a button)
	fn widget_value_drag_drop(layout_target: LayoutTarget, widget_id: u64) -> Message {
		let widget_id = WidgetId(widget_id);
		LayoutMessage::WidgetValueDragDrop { layout_target, widget_id }.into()
	}

	/// Closes out the current transaction (drag-end / text-commit end), so emits during a slider drag collapse into one history step instead of N
	fn end_transaction() -> Message {
		DocumentMessage::EndTransaction.into()
	}

	fn load_preferences(preferences: Option<String>) -> Message {
		let Some(preferences) = preferences else { return Message::NoOp };
		let Ok(preferences) = serde_json::from_str(&preferences) else {
			log::error!("Failed to deserialize preferences");
			return Message::NoOp;
		};
		PreferencesMessage::Load { preferences }.into()
	}

	fn load_document_content(document_id: u64, document: String) -> Message {
		PersistentStateMessage::LoadDocument {
			document_id: DocumentId(document_id),
			document,
		}
		.into()
	}

	fn select_document(document_id: u64) -> Message {
		PortfolioMessage::SelectDocument { document_id: DocumentId(document_id) }.into()
	}

	/// Rename the currently active document.
	fn rename_document(new_name: String) -> Message {
		PortfolioMessage::RenameDocument { new_name }.into()
	}

	fn new_document_dialog() -> Message {
		DialogMessage::RequestNewDocumentDialog.into()
	}

	fn open_file(path: String, content: Vec<u8>) -> Message {
		PortfolioMessage::OpenFile { path: PathBuf::from(path), content }.into()
	}

	fn import_file(path: String, content: Vec<u8>) -> Message {
		PortfolioMessage::ImportFile { path: PathBuf::from(path), content }.into()
	}

	fn trigger_auto_save(document_id: u64) -> Message {
		PortfolioMessage::AutoSaveDocument { document_id: DocumentId(document_id) }.into()
	}

	fn reorder_document(document_id: u64, new_index: usize) -> Message {
		PortfolioMessage::ReorderDocument {
			document_id: DocumentId(document_id),
			new_index,
		}
		.into()
	}

	fn reorder_panel_group_tab(group: u64, old_index: usize, new_index: usize) -> Message {
		PortfolioMessage::ReorderPanelGroupTab {
			group: PanelGroupId(group),
			old_index,
			new_index,
		}
		.into()
	}

	fn move_all_panel_tabs(source_group: u64, target_group: u64, insert_index: usize) -> Message {
		PortfolioMessage::MoveAllPanelTabs {
			source_group: PanelGroupId(source_group),
			target_group: PanelGroupId(target_group),
			insert_index,
		}
		.into()
	}

	fn move_panel_tab(source_group: u64, target_group: u64, insert_index: usize) -> Message {
		PortfolioMessage::MovePanelTab {
			source_group: PanelGroupId(source_group),
			target_group: PanelGroupId(target_group),
			insert_index,
		}
		.into()
	}

	fn set_panel_group_active_tab(group: u64, tab_index: usize) -> Message {
		PortfolioMessage::SetPanelGroupActiveTab {
			group: PanelGroupId(group),
			tab_index,
		}
		.into()
	}

	fn split_panel_group(target_group: u64, direction: DockingSplitDirection, tabs: PanelTypes, active_tab_index: usize) -> Message {
		PortfolioMessage::SplitPanelGroup {
			target_group: PanelGroupId(target_group),
			direction,
			tabs,
			active_tab_index,
		}
		.into()
	}

	fn set_panel_group_sizes(split_path: Vec<u32>, sizes: Vec<f64>) -> Message {
		let split_path = split_path.into_iter().map(|i| i as usize).collect();
		PortfolioMessage::SetPanelGroupSizes { split_path, sizes }.into()
	}

	fn close_document_with_confirmation(document_id: u64) -> Message {
		PortfolioMessage::CloseDocumentWithConfirmation { document_id: DocumentId(document_id) }.into()
	}

	fn request_about_graphite_dialog_with_localized_commit_date(localized_commit_date: String, localized_commit_year: String) -> Message {
		DialogMessage::RequestAboutGraphiteDialogWithLocalizedCommitDate {
			localized_commit_date,
			localized_commit_year,
		}
		.into()
	}

	fn request_licenses_third_party_dialog_with_license_text(license_text: String) -> Message {
		DialogMessage::RequestLicensesThirdPartyDialogWithLicenseText { license_text }.into()
	}

	/// Send new viewport info to the backend
	fn update_viewport(x: f64, y: f64, width: f64, height: f64, scale: f64) -> Message {
		ViewportMessage::Update { x, y, width, height, scale }.into()
	}

	/// Mouse movement within the screenspace bounds of the viewport
	fn on_mouse_move(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Message {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");
		InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys }.into()
	}

	/// Mouse scrolling within the screenspace bounds of the viewport
	fn on_wheel_scroll(x: f64, y: f64, mouse_keys: u8, wheel_delta_x: f64, wheel_delta_y: f64, wheel_delta_z: f64, modifiers: u8) -> Message {
		let mut editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		editor_mouse_state.scroll_delta = ScrollDelta::new(wheel_delta_x, wheel_delta_y, wheel_delta_z);
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");
		InputPreprocessorMessage::WheelScroll { editor_mouse_state, modifier_keys }.into()
	}

	/// A mouse button depressed within screenspace the bounds of the viewport
	fn on_mouse_down(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Message {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");
		InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys }.into()
	}

	/// A mouse button released
	fn on_mouse_up(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Message {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");
		InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys }.into()
	}

	/// Mouse shaken
	fn on_mouse_shake(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Message {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");
		InputPreprocessorMessage::PointerShake { editor_mouse_state, modifier_keys }.into()
	}

	/// Mouse double clicked
	fn on_double_click(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Message {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");
		InputPreprocessorMessage::DoubleClick { editor_mouse_state, modifier_keys }.into()
	}

	/// A keyboard button depressed within screenspace the bounds of the viewport
	fn on_key_down(name: String, modifiers: u8, key_repeat: bool) -> Message {
		let key = translate_key(&name);
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");
		trace!("Key down {key:?}, name: {name}, modifiers: {modifiers:?}, key repeat: {key_repeat}");
		InputPreprocessorMessage::KeyDown { key, key_repeat, modifier_keys }.into()
	}

	/// A keyboard button released
	fn on_key_up(name: String, modifiers: u8, key_repeat: bool) -> Message {
		let key = translate_key(&name);
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");
		trace!("Key up {key:?}, name: {name}, modifiers: {modifier_keys:?}, key repeat: {key_repeat}");
		InputPreprocessorMessage::KeyUp { key, key_repeat, modifier_keys }.into()
	}

	/// A text box was committed
	fn on_change_text(new_text: String, is_left_or_right_click: bool) -> Message {
		TextToolMessage::TextChange { new_text, is_left_or_right_click }.into()
	}

	/// Dialog got dismissed
	fn on_dialog_dismiss() -> Message {
		DialogMessage::Dismiss.into()
	}

	/// A text box was changed
	fn update_bounds(new_text: String) -> Message {
		TextToolMessage::UpdateBounds { new_text }.into()
	}

	/// Update primary color from sRGB bytes (the wire format at the JS boundary).
	fn update_primary_color(color: SRGBA8) -> Message {
		ToolMessage::SelectWorkingColor {
			color: Color::from(color),
			primary: true,
		}
		.into()
	}

	/// Update secondary color from sRGB bytes (the wire format at the JS boundary).
	fn update_secondary_color(color: SRGBA8) -> Message {
		ToolMessage::SelectWorkingColor {
			color: Color::from(color),
			primary: false,
		}
		.into()
	}

	/// Initialize the Rust color picker handler with a starting value (used when the frontend `<ColorPicker />` opens).
	fn open_color_picker(initial_value: FillChoiceUI, allow_none: bool, disabled: bool) -> Message {
		ColorPickerMessage::Open {
			initial_value: FillChoice::from(&initial_value),
			allow_none,
			disabled,
		}
		.into()
	}

	/// Tell the Rust color picker handler that the popover is closing.
	fn close_color_picker() -> Message {
		ColorPickerMessage::Close.into()
	}

	/// Update the color of the currently-edited gradient stop, from sRGB bytes (the wire format at the JS boundary).
	fn update_gradient_stop_color(color: SRGBA8) -> Message {
		GradientToolMessage::UpdateStopColor { color: Color::from(color) }.into()
	}

	/// Start a new undo transaction for gradient stop color editing
	fn start_gradient_stop_color_transaction() -> Message {
		GradientToolMessage::StartTransactionForColorStop.into()
	}

	/// Commit the current gradient stop color transaction (called on pointer-up after each drag/click)
	fn commit_gradient_stop_color_transaction() -> Message {
		GradientToolMessage::CommitTransactionForColorStop.into()
	}

	/// Close the gradient stop color picker and commit any pending transaction
	fn close_gradient_stop_color_picker() -> Message {
		GradientToolMessage::CloseStopColorPicker.into()
	}

	/// Toggle clipping the alpha of a layer to the alpha of the layer below it in the layer stack
	fn clip_layer(id: u64) -> Message {
		DocumentMessage::ClipLayer { id: NodeId(id) }.into()
	}

	/// Modify the layer selection based on the layer which is clicked while holding down the <kbd>Ctrl</kbd> and/or <kbd>Shift</kbd> modifier keys used for range selection behavior
	fn select_layer(id: u64, ctrl: bool, shift: bool) -> Message {
		DocumentMessage::SelectLayer { id: NodeId(id), ctrl, shift }.into()
	}

	/// Deselect all layers
	fn deselect_all_layers() -> Message {
		DocumentMessage::DeselectAllLayers.into()
	}

	/// Move a layer to within a folder and placed down at the given index.
	/// If the folder is `None`, it is inserted into the document root.
	/// If the insert index is `None`, it is inserted at the start of the folder.
	fn move_layer_in_tree(insert_parent_id: Option<u64>, insert_index: Option<usize>) -> Message {
		let insert_parent_id = insert_parent_id.map(NodeId);
		let parent = insert_parent_id.map(LayerNodeIdentifier::new_unchecked).unwrap_or_default();

		DocumentMessage::MoveSelectedLayersTo {
			parent,
			insert_index: insert_index.unwrap_or_default(),
		}
		.into()
	}

	/// Reorder a draggable Properties panel section to the given index among its peers.
	fn reorder_properties_section(node_id: u64, insert_index: usize) -> Message {
		DocumentMessage::ReorderPropertiesSection {
			node_id: NodeId(node_id),
			insert_index,
		}
		.into()
	}

	/// Duplicate the selected layers, placing the copies within the given folder at the given index.
	/// If the folder is `None`, they are inserted into the document root.
	/// If the insert index is `None`, they are inserted at the start of the folder.
	fn duplicate_layer_in_tree(insert_parent_id: Option<u64>, insert_index: Option<usize>) -> Message {
		DocumentMessage::DuplicateSelectedLayersTo {
			parent: insert_parent_id.map(NodeId).map(LayerNodeIdentifier::new_unchecked).unwrap_or_default(),
			insert_index: insert_index.unwrap_or_default(),
		}
		.into()
	}

	/// Set the name for the layer
	fn set_layer_name(id: u64, name: String) -> Message {
		let layer = LayerNodeIdentifier::new_unchecked(NodeId(id));
		NodeGraphMessage::SetDisplayName {
			node_id: layer.to_node(),
			network_path: Vec::new(),
			alias: name,
			skip_adding_history_step: false,
		}
		.into()
	}

	/// Translates document (in viewport coords)
	fn pan_canvas_abort_prepare(x_not_y_axis: bool) -> Message {
		NavigationMessage::CanvasPanAbortPrepare { x_not_y_axis }.into()
	}

	fn pan_canvas_abort(x_not_y_axis: bool) -> Message {
		NavigationMessage::CanvasPanAbort { x_not_y_axis }.into()
	}

	/// Translates document (in viewport coords)
	fn pan_canvas(delta_x: f64, delta_y: f64) -> Message {
		NavigationMessage::CanvasPan { delta: (delta_x, delta_y).into() }.into()
	}

	/// Translates document (in viewport coords)
	fn pan_canvas_by_fraction(delta_x: f64, delta_y: f64) -> Message {
		NavigationMessage::CanvasPanByViewportFraction { delta: (delta_x, delta_y).into() }.into()
	}

	/// Merge the selected nodes into a subnetwork
	fn merge_selected_nodes() -> Message {
		NodeGraphMessage::MergeSelectedNodes.into()
	}

	/// Toggle lock state of all selected layers
	fn toggle_selected_locked() -> Message {
		NodeGraphMessage::ToggleSelectedLocked.into()
	}

	/// Creates a new document node in the node graph
	fn create_node(node_type: Any, x: i32, y: i32) -> Message {
		let id = NodeId::new();
		NodeGraphMessage::CreateNodeFromContextMenu {
			node_id: Some(id),
			node_type: node_type.cast(),
			xy: Some((x / 24, y / 24)),
			add_transaction: true,
		}
		.into()
	}

	/// Respond to selection read
	fn read_selection(content: Option<String>, cut: bool) -> Message {
		ClipboardMessage::ReadSelection { content, cut }.into()
	}

	/// Paste from a serialized JSON representation
	fn paste_text(data: String) -> Message {
		ClipboardMessage::ReadClipboard {
			content: ClipboardContentRaw::Text(data),
		}
		.into()
	}

	/// Pastes an image
	fn paste_image(
		name: Option<String>,
		image_data: Vec<u8>,
		width: u32,
		height: u32,
		mouse_x: Option<f64>,
		mouse_y: Option<f64>,
		insert_parent_id: Option<u64>,
		insert_index: Option<usize>,
	) -> Message {
		let mouse = mouse_x.and_then(|x| mouse_y.map(|y| (x, y)));
		let image = graphene_std::raster::Image::from_image_data(&image_data, width, height);

		let parent_and_insert_index = if let (Some(insert_parent_id), Some(insert_index)) = (insert_parent_id, insert_index) {
			let insert_parent_id = NodeId(insert_parent_id);
			let parent = LayerNodeIdentifier::new_unchecked(insert_parent_id);
			Some((parent, insert_index))
		} else {
			None
		};

		PortfolioMessage::InsertImage {
			name,
			image,
			mouse,
			parent_and_insert_index,
		}
		.into()
	}

	/// Pastes an SVG given its string representation
	fn paste_svg(name: Option<String>, svg: String, mouse_x: Option<f64>, mouse_y: Option<f64>, insert_parent_id: Option<u64>, insert_index: Option<usize>) -> Message {
		let mouse = mouse_x.and_then(|x| mouse_y.map(|y| (x, y)));

		let parent_and_insert_index = if let (Some(insert_parent_id), Some(insert_index)) = (insert_parent_id, insert_index) {
			let insert_parent_id = NodeId(insert_parent_id);
			let parent = LayerNodeIdentifier::new_unchecked(insert_parent_id);
			Some((parent, insert_index))
		} else {
			None
		};

		PortfolioMessage::InsertSvg {
			name,
			svg,
			mouse,
			parent_and_insert_index,
		}
		.into()
	}

	/// Toggle visibility of a layer or node given its node ID
	fn toggle_node_visibility_layer_panel(id: u64) -> Message {
		NodeGraphMessage::ToggleVisibility {
			node_id: NodeId(id),
			network_path: Vec::new(),
		}
		.into()
	}

	/// Pin or unpin a node given its node ID
	fn set_node_pinned(id: u64, pinned: bool) -> Message {
		DocumentMessage::SetNodePinned { node_id: NodeId(id), pinned }.into()
	}

	/// Collapse or expand a node's section in the Properties panel
	fn toggle_node_properties_section_expanded(id: u64) -> Message {
		DocumentMessage::ToggleNodePropertiesSectionExpanded { node_id: NodeId(id) }.into()
	}

	/// Delete a layer or node given its node ID
	fn delete_node(id: u64) -> Message {
		DocumentMessage::DeleteNode { node_id: NodeId(id) }.into()
	}

	/// Toggle lock state of a layer from the layer list
	fn toggle_layer_lock(node_id: u64) -> Message {
		NodeGraphMessage::ToggleLocked {
			node_id: NodeId(node_id),
			network_path: Vec::new(),
		}
		.into()
	}

	/// Toggle expansions state of a layer from the layer list
	fn toggle_layer_expansion(tree_path: Vec<u64>, recursive: bool) -> Message {
		let tree_path = tree_path.into_iter().map(NodeId).collect();
		DocumentMessage::ToggleLayerExpansion { tree_path, recursive }.into()
	}

	/// Set the active panel to the most recently clicked panel
	fn set_active_panel(panel: String) -> Message {
		DocumentMessage::SetActivePanel { active_panel: panel.into() }.into()
	}

	/// Toggle display type for a layer
	fn set_to_node_or_layer(id: u64, is_layer: bool) -> Message {
		DocumentMessage::SetToNodeOrLayer { node_id: NodeId(id), is_layer }.into()
	}

	/// Set the name of an import or export
	fn set_import_name(index: usize, name: String) -> Message {
		NodeGraphMessage::SetImportExportName {
			name,
			index: ImportOrExport::Import(index),
		}
		.into()
	}

	/// Set the name of an export
	fn set_export_name(index: usize, name: String) -> Message {
		NodeGraphMessage::SetImportExportName {
			name,
			index: ImportOrExport::Export(index),
		}
		.into()
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(from_wasm_abi)]
pub struct Any(#[tsify(type = "any")] serde_json::Value);
impl Any {
	#[cfg(feature = "editor")]
	pub(crate) fn cast<T: for<'de> Deserialize<'de>>(&self) -> T {
		serde_json::from_value(self.0.clone()).unwrap()
	}
}

macro_rules! editor_proxy_types {
	($($name:ident = $real:ty;)*) => {
		$(
			#[cfg(feature = "editor")]
			pub type $name = $real;
			#[cfg(not(feature = "editor"))]
			pub type $name = Any;
		)*
	};
}

editor_proxy_types! {
	LayoutTarget = editor::messages::layout::utility_types::layout_widget::LayoutTarget;
	DockingSplitDirection = editor::messages::portfolio::utility_types::DockingSplitDirection;
	PanelTypes = Vec<editor::messages::portfolio::utility_types::PanelType>;
	SRGBA8 = graphene_std::color::SRGBA8;
	FillChoiceUI = graphene_std::vector::style::FillChoiceUI;
}
