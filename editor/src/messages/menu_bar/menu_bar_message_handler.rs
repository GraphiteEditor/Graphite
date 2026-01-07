use crate::messages::debug::utility_types::MessageLoggingVerbosity;
use crate::messages::input_mapper::utility_types::macros::action_shortcut;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis, GroupFolderType};
use crate::messages::prelude::*;
use graphene_std::path_bool::BooleanOperation;

#[derive(Debug, Clone, Default, ExtractField)]
pub struct MenuBarMessageHandler {
	pub has_active_document: bool,
	pub canvas_tilted: bool,
	pub canvas_flipped: bool,
	pub rulers_visible: bool,
	pub node_graph_open: bool,
	pub has_selected_nodes: bool,
	pub has_selected_layers: bool,
	pub has_selection_history: (bool, bool),
	pub message_logging_verbosity: MessageLoggingVerbosity,
	pub reset_node_definitions_on_open: bool,
	pub make_path_editable_is_allowed: bool,
	pub data_panel_open: bool,
	pub layers_panel_open: bool,
	pub properties_panel_open: bool,
}

#[message_handler_data]
impl MessageHandler<MenuBarMessage, ()> for MenuBarMessageHandler {
	fn process_message(&mut self, message: MenuBarMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			MenuBarMessage::SendLayout => {
				self.send_layout(responses, LayoutTarget::MenuBar);
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(MenuBarMessageDiscriminant;)
	}
}

impl LayoutHolder for MenuBarMessageHandler {
	fn layout(&self) -> Layout {
		let no_active_document = !self.has_active_document;
		let node_graph_open = self.node_graph_open;
		let has_selected_nodes = self.has_selected_nodes;
		let has_selected_layers = self.has_selected_layers;
		let has_selection_history = self.has_selection_history;
		let message_logging_verbosity_off = self.message_logging_verbosity == MessageLoggingVerbosity::Off;
		let message_logging_verbosity_names = self.message_logging_verbosity == MessageLoggingVerbosity::Names;
		let message_logging_verbosity_contents = self.message_logging_verbosity == MessageLoggingVerbosity::Contents;
		let reset_node_definitions_on_open = self.reset_node_definitions_on_open;
		let make_path_editable_is_allowed = self.make_path_editable_is_allowed;

		let about = MenuListEntry::new("About Graphite…")
			.label({
				#[cfg(not(target_os = "macos"))]
				{
					"About Graphite…"
				}
				#[cfg(target_os = "macos")]
				{
					"About Graphite"
				}
			})
			.icon("GraphiteLogo")
			.on_commit(|_| DialogMessage::RequestAboutGraphiteDialog.into());
		let preferences = MenuListEntry::new("Preferences…")
			.label("Preferences…")
			.icon("Settings")
			.tooltip_shortcut(action_shortcut!(DialogMessageDiscriminant::RequestPreferencesDialog))
			.on_commit(|_| DialogMessage::RequestPreferencesDialog.into());

		let menu_bar_buttons = vec![
			#[cfg(not(target_os = "macos"))]
			TextButton::new("Graphite")
				.label("")
				.flush(true)
				.icon(Some("GraphiteLogo".into()))
				.on_commit(|_| FrontendMessage::TriggerVisitLink { url: "https://graphite.art".into() }.into())
				.widget_instance(),
			#[cfg(target_os = "macos")]
			TextButton::new("Graphite")
				.label("")
				.flush(true)
				.menu_list_children(vec![
					vec![about],
					vec![preferences],
					vec![
						MenuListEntry::new("Hide Graphite")
							.label("Hide Graphite")
							.tooltip_shortcut(action_shortcut!(AppWindowMessageDiscriminant::Hide))
							.on_commit(|_| AppWindowMessage::Hide.into()),
						MenuListEntry::new("Hide Others")
							.label("Hide Others")
							.tooltip_shortcut(action_shortcut!(AppWindowMessageDiscriminant::HideOthers))
							.on_commit(|_| AppWindowMessage::HideOthers.into()),
						MenuListEntry::new("Show All")
							.label("Show All")
							.tooltip_shortcut(action_shortcut!(AppWindowMessageDiscriminant::ShowAll))
							.on_commit(|_| AppWindowMessage::ShowAll.into()),
					],
					vec![
						MenuListEntry::new("Quit Graphite")
							.label("Quit Graphite")
							.tooltip_shortcut(action_shortcut!(AppWindowMessageDiscriminant::Close))
							.on_commit(|_| AppWindowMessage::Close.into()),
					],
				])
				.widget_instance(),
			TextButton::new("File")
				.label("File")
				.flush(true)
				.menu_list_children(vec![
					vec![
						MenuListEntry::new("New…")
							.label("New…")
							.icon("File")
							.on_commit(|_| DialogMessage::RequestNewDocumentDialog.into())
							.tooltip_shortcut(action_shortcut!(DialogMessageDiscriminant::RequestNewDocumentDialog)),
						MenuListEntry::new("Open…")
							.label("Open…")
							.icon("Folder")
							.tooltip_shortcut(action_shortcut!(PortfolioMessageDiscriminant::OpenDocument))
							.on_commit(|_| PortfolioMessage::OpenDocument.into()),
						MenuListEntry::new("Open Demo Artwork…")
							.label("Open Demo Artwork…")
							.icon("Image")
							.on_commit(|_| DialogMessage::RequestDemoArtworkDialog.into()),
					],
					vec![
						MenuListEntry::new("Close")
							.label("Close")
							.icon("Close")
							.tooltip_shortcut(action_shortcut!(PortfolioMessageDiscriminant::CloseActiveDocumentWithConfirmation))
							.on_commit(|_| PortfolioMessage::CloseActiveDocumentWithConfirmation.into())
							.disabled(no_active_document),
						MenuListEntry::new("Close All")
							.label("Close All")
							.icon("CloseAll")
							.tooltip_shortcut(action_shortcut!(PortfolioMessageDiscriminant::CloseAllDocumentsWithConfirmation))
							.on_commit(|_| PortfolioMessage::CloseAllDocumentsWithConfirmation.into())
							.disabled(no_active_document),
					],
					vec![
						MenuListEntry::new("Save")
							.label("Save")
							.icon("Save")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::SaveDocument))
							.on_commit(|_| DocumentMessage::SaveDocument.into())
							.disabled(no_active_document),
						#[cfg(not(target_family = "wasm"))]
						MenuListEntry::new("Save As…")
							.label("Save As…")
							.icon("Save")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::SaveDocumentAs))
							.on_commit(|_| DocumentMessage::SaveDocumentAs.into())
							.disabled(no_active_document),
					],
					vec![
						MenuListEntry::new("Import…")
							.label("Import…")
							.icon("FileImport")
							.tooltip_shortcut(action_shortcut!(PortfolioMessageDiscriminant::Import))
							.on_commit(|_| PortfolioMessage::Import.into()),
						MenuListEntry::new("Export…")
							.label("Export…")
							.icon("FileExport")
							.tooltip_shortcut(action_shortcut!(DialogMessageDiscriminant::RequestExportDialog))
							.on_commit(|_| DialogMessage::RequestExportDialog.into())
							.disabled(no_active_document),
					],
					#[cfg(not(target_os = "macos"))]
					vec![preferences],
				])
				.widget_instance(),
			TextButton::new("Edit")
				.label("Edit")
				.flush(true)
				.menu_list_children(vec![
					vec![
						MenuListEntry::new("Undo")
							.label("Undo")
							.icon("HistoryUndo")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::Undo))
							.on_commit(|_| DocumentMessage::Undo.into())
							.disabled(no_active_document),
						MenuListEntry::new("Redo")
							.label("Redo")
							.icon("HistoryRedo")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::Redo))
							.on_commit(|_| DocumentMessage::Redo.into())
							.disabled(no_active_document),
					],
					vec![
						MenuListEntry::new("Cut")
							.label("Cut")
							.icon("Cut")
							.tooltip_shortcut(action_shortcut!(ClipboardMessageDiscriminant::Cut))
							.on_commit(|_| ClipboardMessage::Cut.into())
							.disabled(no_active_document || !has_selected_layers),
						MenuListEntry::new("Copy")
							.label("Copy")
							.icon("Copy")
							.tooltip_shortcut(action_shortcut!(ClipboardMessageDiscriminant::Copy))
							.on_commit(|_| ClipboardMessage::Copy.into())
							.disabled(no_active_document || !has_selected_layers),
						MenuListEntry::new("Paste")
							.label("Paste")
							.icon("Paste")
							.tooltip_shortcut(action_shortcut!(ClipboardMessageDiscriminant::Paste))
							.on_commit(|_| ClipboardMessage::Paste.into())
							.disabled(no_active_document),
					],
					vec![
						MenuListEntry::new("Duplicate")
							.label("Duplicate")
							.icon("Copy")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::DuplicateSelectedLayers))
							.on_commit(|_| DocumentMessage::DuplicateSelectedLayers.into())
							.disabled(no_active_document || !has_selected_nodes),
						MenuListEntry::new("Delete")
							.label("Delete")
							.icon("Trash")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::DeleteSelectedLayers))
							.on_commit(|_| DocumentMessage::DeleteSelectedLayers.into())
							.disabled(no_active_document || !has_selected_nodes),
					],
					vec![
						MenuListEntry::new("Convert to Infinite Canvas")
							.label("Convert to Infinite Canvas")
							.icon("Artboard")
							.on_commit(|_| DocumentMessage::RemoveArtboards.into())
							.disabled(no_active_document),
					],
				])
				.widget_instance(),
			TextButton::new("Layer")
				.label("Layer")
				.flush(true)
				.menu_list_children(vec![
					vec![
						MenuListEntry::new("New")
							.label("New")
							.icon("NewLayer")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::CreateEmptyFolder))
							.on_commit(|_| DocumentMessage::CreateEmptyFolder.into())
							.disabled(no_active_document),
					],
					vec![
						MenuListEntry::new("Group")
							.label("Group")
							.icon("Folder")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::GroupSelectedLayers))
							.on_commit(|_| {
								DocumentMessage::GroupSelectedLayers {
									group_folder_type: GroupFolderType::Layer,
								}
								.into()
							})
							.disabled(no_active_document || !has_selected_layers),
						MenuListEntry::new("Ungroup")
							.label("Ungroup")
							.icon("FolderOpen")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::UngroupSelectedLayers))
							.on_commit(|_| DocumentMessage::UngroupSelectedLayers.into())
							.disabled(no_active_document || !has_selected_layers),
					],
					vec![
						MenuListEntry::new("Hide/Show")
							.label("Hide/Show")
							.icon("EyeHide")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::ToggleSelectedVisibility))
							.on_commit(|_| DocumentMessage::ToggleSelectedVisibility.into())
							.disabled(no_active_document || !has_selected_layers),
						MenuListEntry::new("Lock/Unlock")
							.label("Lock/Unlock")
							.icon("PadlockLocked")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::ToggleSelectedLocked))
							.on_commit(|_| DocumentMessage::ToggleSelectedLocked.into())
							.disabled(no_active_document || !has_selected_layers),
					],
					vec![
						MenuListEntry::new("Grab")
							.label("Grab")
							.icon("TransformationGrab")
							.tooltip_shortcut(action_shortcut!(TransformLayerMessageDiscriminant::BeginGrab))
							.on_commit(|_| TransformLayerMessage::BeginGrab.into())
							.disabled(no_active_document || !has_selected_layers),
						MenuListEntry::new("Rotate")
							.label("Rotate")
							.icon("TransformationRotate")
							.tooltip_shortcut(action_shortcut!(TransformLayerMessageDiscriminant::BeginRotate))
							.on_commit(|_| TransformLayerMessage::BeginRotate.into())
							.disabled(no_active_document || !has_selected_layers),
						MenuListEntry::new("Scale")
							.label("Scale")
							.icon("TransformationScale")
							.tooltip_shortcut(action_shortcut!(TransformLayerMessageDiscriminant::BeginScale))
							.on_commit(|_| TransformLayerMessage::BeginScale.into())
							.disabled(no_active_document || !has_selected_layers),
					],
					vec![
						MenuListEntry::new("Arrange")
							.label("Arrange")
							.icon("StackHollow")
							.disabled(no_active_document || !has_selected_layers)
							.children(vec![
								vec![
									MenuListEntry::new("Raise To Front")
										.label("Raise To Front")
										.icon("Stack")
										.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::SelectedLayersRaiseToFront))
										.on_commit(|_| DocumentMessage::SelectedLayersRaiseToFront.into())
										.disabled(no_active_document || !has_selected_layers),
									MenuListEntry::new("Raise")
										.label("Raise")
										.icon("StackRaise")
										.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::SelectedLayersRaise))
										.on_commit(|_| DocumentMessage::SelectedLayersRaise.into())
										.disabled(no_active_document || !has_selected_layers),
									MenuListEntry::new("Lower")
										.label("Lower")
										.icon("StackLower")
										.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::SelectedLayersLower))
										.on_commit(|_| DocumentMessage::SelectedLayersLower.into())
										.disabled(no_active_document || !has_selected_layers),
									MenuListEntry::new("Lower to Back")
										.label("Lower to Back")
										.icon("StackBottom")
										.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::SelectedLayersLowerToBack))
										.on_commit(|_| DocumentMessage::SelectedLayersLowerToBack.into())
										.disabled(no_active_document || !has_selected_layers),
								],
								vec![
									MenuListEntry::new("Reverse")
										.label("Reverse")
										.icon("StackReverse")
										.on_commit(|_| DocumentMessage::SelectedLayersReverse.into())
										.disabled(no_active_document || !has_selected_layers),
								],
							]),
						MenuListEntry::new("Align")
							.label("Align")
							.icon("AlignVerticalCenter")
							.disabled(no_active_document || !has_selected_layers)
							.children(vec![
								vec![
									MenuListEntry::new("Align Left")
										.label("Align Left")
										.icon("AlignLeft")
										.on_commit(|_| {
											DocumentMessage::AlignSelectedLayers {
												axis: AlignAxis::X,
												aggregate: AlignAggregate::Min,
											}
											.into()
										})
										.disabled(no_active_document || !has_selected_layers),
									MenuListEntry::new("Align Horizontal Center")
										.label("Align Horizontal Center")
										.icon("AlignHorizontalCenter")
										.on_commit(|_| {
											DocumentMessage::AlignSelectedLayers {
												axis: AlignAxis::X,
												aggregate: AlignAggregate::Center,
											}
											.into()
										})
										.disabled(no_active_document || !has_selected_layers),
									MenuListEntry::new("Align Right")
										.label("Align Right")
										.icon("AlignRight")
										.on_commit(|_| {
											DocumentMessage::AlignSelectedLayers {
												axis: AlignAxis::X,
												aggregate: AlignAggregate::Max,
											}
											.into()
										})
										.disabled(no_active_document || !has_selected_layers),
								],
								vec![
									MenuListEntry::new("Align Top")
										.label("Align Top")
										.icon("AlignTop")
										.on_commit(|_| {
											DocumentMessage::AlignSelectedLayers {
												axis: AlignAxis::Y,
												aggregate: AlignAggregate::Min,
											}
											.into()
										})
										.disabled(no_active_document || !has_selected_layers),
									MenuListEntry::new("Align Vertical Center")
										.label("Align Vertical Center")
										.icon("AlignVerticalCenter")
										.on_commit(|_| {
											DocumentMessage::AlignSelectedLayers {
												axis: AlignAxis::Y,
												aggregate: AlignAggregate::Center,
											}
											.into()
										})
										.disabled(no_active_document || !has_selected_layers),
									MenuListEntry::new("Align Bottom")
										.label("Align Bottom")
										.icon("AlignBottom")
										.on_commit(|_| {
											DocumentMessage::AlignSelectedLayers {
												axis: AlignAxis::Y,
												aggregate: AlignAggregate::Max,
											}
											.into()
										})
										.disabled(no_active_document || !has_selected_layers),
								],
							]),
						MenuListEntry::new("Flip")
							.label("Flip")
							.icon("FlipVertical")
							.disabled(no_active_document || !has_selected_layers)
							.children(vec![vec![
								MenuListEntry::new("Flip Horizontal")
									.label("Flip Horizontal")
									.icon("FlipHorizontal")
									.on_commit(|_| DocumentMessage::FlipSelectedLayers { flip_axis: FlipAxis::X }.into())
									.disabled(no_active_document || !has_selected_layers),
								MenuListEntry::new("Flip Vertical")
									.label("Flip Vertical")
									.icon("FlipVertical")
									.on_commit(|_| DocumentMessage::FlipSelectedLayers { flip_axis: FlipAxis::Y }.into())
									.disabled(no_active_document || !has_selected_layers),
							]]),
						MenuListEntry::new("Turn")
							.label("Turn")
							.icon("TurnPositive90")
							.disabled(no_active_document || !has_selected_layers)
							.children(vec![vec![
								MenuListEntry::new("Turn -90°")
									.label("Turn -90°")
									.icon("TurnNegative90")
									.on_commit(|_| DocumentMessage::RotateSelectedLayers { degrees: -90. }.into())
									.disabled(no_active_document || !has_selected_layers),
								MenuListEntry::new("Turn 90°")
									.label("Turn 90°")
									.icon("TurnPositive90")
									.on_commit(|_| DocumentMessage::RotateSelectedLayers { degrees: 90. }.into())
									.disabled(no_active_document || !has_selected_layers),
							]]),
						MenuListEntry::new("Boolean")
							.label("Boolean")
							.icon("BooleanSubtractFront")
							.disabled(no_active_document || !has_selected_layers)
							.children(vec![vec![
								MenuListEntry::new("Union")
									.label("Union")
									.icon("BooleanUnion")
									.on_commit(|_| {
										let group_folder_type = GroupFolderType::BooleanOperation(BooleanOperation::Union);
										DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
									})
									.disabled(no_active_document || !has_selected_layers),
								MenuListEntry::new("Subtract Front")
									.label("Subtract Front")
									.icon("BooleanSubtractFront")
									.on_commit(|_| {
										let group_folder_type = GroupFolderType::BooleanOperation(BooleanOperation::SubtractFront);
										DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
									})
									.disabled(no_active_document || !has_selected_layers),
								MenuListEntry::new("Subtract Back")
									.label("Subtract Back")
									.icon("BooleanSubtractBack")
									.on_commit(|_| {
										let group_folder_type = GroupFolderType::BooleanOperation(BooleanOperation::SubtractBack);
										DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
									})
									.disabled(no_active_document || !has_selected_layers),
								MenuListEntry::new("Intersect")
									.label("Intersect")
									.icon("BooleanIntersect")
									.on_commit(|_| {
										let group_folder_type = GroupFolderType::BooleanOperation(BooleanOperation::Intersect);
										DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
									})
									.disabled(no_active_document || !has_selected_layers),
								MenuListEntry::new("Difference")
									.label("Difference")
									.icon("BooleanDifference")
									.on_commit(|_| {
										let group_folder_type = GroupFolderType::BooleanOperation(BooleanOperation::Difference);
										DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
									})
									.disabled(no_active_document || !has_selected_layers),
							]]),
					],
					vec![
						MenuListEntry::new("Make Path Editable")
							.label("Make Path Editable")
							.icon("NodeShape")
							.on_commit(|_| NodeGraphMessage::AddPathNode.into())
							.disabled(!make_path_editable_is_allowed),
					],
				])
				.widget_instance(),
			TextButton::new("Select")
				.label("Select")
				.flush(true)
				.menu_list_children(vec![
					vec![
						MenuListEntry::new("Select All")
							.label("Select All")
							.icon("SelectAll")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::SelectAllLayers))
							.on_commit(|_| DocumentMessage::SelectAllLayers.into())
							.disabled(no_active_document),
						MenuListEntry::new("Deselect All")
							.label("Deselect All")
							.icon("DeselectAll")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::DeselectAllLayers))
							.on_commit(|_| DocumentMessage::DeselectAllLayers.into())
							.disabled(no_active_document || !has_selected_nodes),
						MenuListEntry::new("Select Parent")
							.label("Select Parent")
							.icon("SelectParent")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::SelectParentLayer))
							.on_commit(|_| DocumentMessage::SelectParentLayer.into())
							.disabled(no_active_document || !has_selected_nodes),
					],
					vec![
						MenuListEntry::new("Previous Selection")
							.label("Previous Selection")
							.icon("HistoryUndo")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::SelectionStepBack))
							.on_commit(|_| DocumentMessage::SelectionStepBack.into())
							.disabled(!has_selection_history.0),
						MenuListEntry::new("Next Selection")
							.label("Next Selection")
							.icon("HistoryRedo")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::SelectionStepForward))
							.on_commit(|_| DocumentMessage::SelectionStepForward.into())
							.disabled(!has_selection_history.1),
					],
				])
				.widget_instance(),
			TextButton::new("View")
				.label("View")
				.flush(true)
				.menu_list_children(vec![
					vec![
						MenuListEntry::new("Tilt")
							.label("Tilt")
							.icon("Tilt")
							.tooltip_shortcut(action_shortcut!(NavigationMessageDiscriminant::BeginCanvasTilt))
							.on_commit(|_| NavigationMessage::BeginCanvasTilt { was_dispatched_from_menu: true }.into())
							.disabled(no_active_document || node_graph_open),
						MenuListEntry::new("Reset Tilt")
							.label("Reset Tilt")
							.icon("TiltReset")
							.tooltip_shortcut(action_shortcut!(NavigationMessageDiscriminant::CanvasTiltSet))
							.on_commit(|_| NavigationMessage::CanvasTiltSet { angle_radians: 0.into() }.into())
							.disabled(no_active_document || node_graph_open || !self.canvas_tilted),
					],
					vec![
						MenuListEntry::new("Zoom In")
							.label("Zoom In")
							.icon("ZoomIn")
							.tooltip_shortcut(action_shortcut!(NavigationMessageDiscriminant::CanvasZoomIncrease))
							.on_commit(|_| NavigationMessage::CanvasZoomIncrease { center_on_mouse: false }.into())
							.disabled(no_active_document),
						MenuListEntry::new("Zoom Out")
							.label("Zoom Out")
							.icon("ZoomOut")
							.tooltip_shortcut(action_shortcut!(NavigationMessageDiscriminant::CanvasZoomDecrease))
							.on_commit(|_| NavigationMessage::CanvasZoomDecrease { center_on_mouse: false }.into())
							.disabled(no_active_document),
						MenuListEntry::new("Zoom to Selection")
							.label("Zoom to Selection")
							.icon("FrameSelected")
							.tooltip_shortcut(action_shortcut!(NavigationMessageDiscriminant::FitViewportToSelection))
							.on_commit(|_| NavigationMessage::FitViewportToSelection.into())
							.disabled(no_active_document || !has_selected_layers),
						MenuListEntry::new("Zoom to Fit")
							.label("Zoom to Fit")
							.icon("FrameAll")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::ZoomCanvasToFitAll))
							.on_commit(|_| DocumentMessage::ZoomCanvasToFitAll.into())
							.disabled(no_active_document),
						MenuListEntry::new("Zoom to 100%")
							.label("Zoom to 100%")
							.icon("Zoom1x")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::ZoomCanvasTo100Percent))
							.on_commit(|_| DocumentMessage::ZoomCanvasTo100Percent.into())
							.disabled(no_active_document),
						MenuListEntry::new("Zoom to 200%")
							.label("Zoom to 200%")
							.icon("Zoom2x")
							.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::ZoomCanvasTo200Percent))
							.on_commit(|_| DocumentMessage::ZoomCanvasTo200Percent.into())
							.disabled(no_active_document),
					],
					vec![
						MenuListEntry::new("Flip")
							.label("Flip")
							.icon(if self.canvas_flipped { "CheckboxChecked" } else { "CheckboxUnchecked" })
							.tooltip_shortcut(action_shortcut!(NavigationMessageDiscriminant::CanvasFlip))
							.on_commit(|_| NavigationMessage::CanvasFlip.into())
							.disabled(no_active_document || node_graph_open),
					],
					vec![
						MenuListEntry::new("Rulers")
							.label("Rulers")
							.icon(if self.rulers_visible { "CheckboxChecked" } else { "CheckboxUnchecked" })
							.tooltip_shortcut(action_shortcut!(PortfolioMessageDiscriminant::ToggleRulers))
							.on_commit(|_| PortfolioMessage::ToggleRulers.into())
							.disabled(no_active_document),
					],
				])
				.widget_instance(),
			TextButton::new("Window")
				.label("Window")
				.flush(true)
				.menu_list_children(vec![
					vec![
						MenuListEntry::new("Properties")
							.label("Properties")
							.icon(if self.properties_panel_open { "CheckboxChecked" } else { "CheckboxUnchecked" })
							.tooltip_shortcut(action_shortcut!(PortfolioMessageDiscriminant::TogglePropertiesPanelOpen))
							.on_commit(|_| PortfolioMessage::TogglePropertiesPanelOpen.into()),
						MenuListEntry::new("Layers")
							.label("Layers")
							.icon(if self.layers_panel_open { "CheckboxChecked" } else { "CheckboxUnchecked" })
							.tooltip_shortcut(action_shortcut!(PortfolioMessageDiscriminant::ToggleLayersPanelOpen))
							.on_commit(|_| PortfolioMessage::ToggleLayersPanelOpen.into()),
					],
					vec![
						MenuListEntry::new("Data")
							.label("Data")
							.icon(if self.data_panel_open { "CheckboxChecked" } else { "CheckboxUnchecked" })
							.tooltip_shortcut(action_shortcut!(PortfolioMessageDiscriminant::ToggleDataPanelOpen))
							.on_commit(|_| PortfolioMessage::ToggleDataPanelOpen.into()),
					],
				])
				.widget_instance(),
			TextButton::new("Help")
				.label("Help")
				.flush(true)
				.menu_list_children(vec![
					#[cfg(not(target_os = "macos"))]
					vec![about],
					vec![
						MenuListEntry::new("Donate to Graphite").label("Donate to Graphite").icon("Heart").on_commit(|_| {
							FrontendMessage::TriggerVisitLink {
								url: "https://graphite.art/donate/".into(),
							}
							.into()
						}),
						MenuListEntry::new("User Manual").label("User Manual").icon("UserManual").on_commit(|_| {
							FrontendMessage::TriggerVisitLink {
								url: "https://graphite.art/learn/".into(),
							}
							.into()
						}),
						MenuListEntry::new("Report a Bug").label("Report a Bug").icon("Bug").on_commit(|_| {
							FrontendMessage::TriggerVisitLink {
								url: "https://github.com/GraphiteEditor/Graphite/issues/new".into(),
							}
							.into()
						}),
						MenuListEntry::new("Visit on GitHub").label("Visit on GitHub").icon("Website").on_commit(|_| {
							FrontendMessage::TriggerVisitLink {
								url: "https://github.com/GraphiteEditor/Graphite".into(),
							}
							.into()
						}),
					],
					vec![MenuListEntry::new("Developer Debug").label("Developer Debug").icon("Code").children(vec![
						vec![
							MenuListEntry::new("Reset Nodes to Definitions on Open")
								.label("Reset Nodes to Definitions on Open")
								.icon(if reset_node_definitions_on_open { "CheckboxChecked" } else { "CheckboxUnchecked" })
								.on_commit(|_| PortfolioMessage::ToggleResetNodesToDefinitionsOnOpen.into()),
						],
						vec![
							MenuListEntry::new("Print Trace Logs")
								.label("Print Trace Logs")
								.icon(if log::max_level() == log::LevelFilter::Trace { "CheckboxChecked" } else { "CheckboxUnchecked" })
								.on_commit(|_| DebugMessage::ToggleTraceLogs.into()),
							MenuListEntry::new("Print Messages: Off")
								.label("Print Messages: Off")
								.icon(if message_logging_verbosity_off {
									#[cfg(not(target_os = "macos"))]
									{
										"SmallDot".to_string()
									}
									#[cfg(target_os = "macos")]
									{
										"CheckboxChecked".to_string()
									}
								} else { Default::default() })
								.tooltip_shortcut(action_shortcut!(DebugMessageDiscriminant::MessageOff))
								.on_commit(|_| DebugMessage::MessageOff.into()),
							MenuListEntry::new("Print Messages: Only Names")
								.label("Print Messages: Only Names")
								.icon(if message_logging_verbosity_names {
									#[cfg(not(target_os = "macos"))]
									{
										"SmallDot".to_string()
									}
									#[cfg(target_os = "macos")]
									{
										"CheckboxChecked".to_string()
									}
								} else { Default::default() })
								.tooltip_shortcut(action_shortcut!(DebugMessageDiscriminant::MessageNames))
								.on_commit(|_| DebugMessage::MessageNames.into()),
							MenuListEntry::new("Print Messages: Full Contents")
								.label("Print Messages: Full Contents")
								.icon(if message_logging_verbosity_contents {
									#[cfg(not(target_os = "macos"))]
									{
										"SmallDot".to_string()
									}
									#[cfg(target_os = "macos")]
									{
										"CheckboxChecked".to_string()
									}
								} else { Default::default() })
								.tooltip_shortcut(action_shortcut!(DebugMessageDiscriminant::MessageContents))
								.on_commit(|_| DebugMessage::MessageContents.into()),
						],
						vec![MenuListEntry::new("Trigger a Crash").label("Trigger a Crash").icon("Warning").on_commit(|_| panic!())],
					])],
				])
				.widget_instance(),
		];

		Layout(vec![LayoutGroup::Row { widgets: menu_bar_buttons }])
	}
}
