use super::MenuBarMessage;
use crate::input::input_mapper::FutureKeyMapping;
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::*;
use crate::message_prelude::*;

use std::collections::VecDeque;

#[derive(Debug, Clone, Default)]
pub struct MenuBarMessageHandler {}

impl MessageHandler<MenuBarMessage, ()> for MenuBarMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: MenuBarMessage, _data: (), responses: &mut VecDeque<Message>) {
		use MenuBarMessage::*;

		#[remain::sorted]
		match message {
			SendLayout => self.register_properties(responses, LayoutTarget::MenuBar),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(MenuBarMessageDiscriminant;)
	}
}

macro_rules! action_shortcut {
	($action:expr) => {
		Some(FutureKeyMapping::new($action.into()))
	};
}

impl PropertyHolder for MenuBarMessageHandler {
	fn properties(&self) -> Layout {
		Layout::MenuLayout(MenuLayout::new(vec![
			MenuColumn {
				label: "File".into(),
				children: MenuEntryGroups(vec![
					vec![
						MenuEntry {
							label: "New…".into(),
							icon: Some("File".into()),
							action: MenuEntry::create_action(|_| DialogMessage::RequestNewDocumentDialog.into()),
							shortcut: action_shortcut!(DialogMessageDiscriminant::RequestNewDocumentDialog),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyN]),
							children: MenuEntryGroups::empty(),
						},
						MenuEntry {
							label: "Open…".into(),
							shortcut: action_shortcut!(PortfolioMessageDiscriminant::OpenDocument),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyO]),
							action: MenuEntry::create_action(|_| PortfolioMessage::OpenDocument.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Open Recent".into(),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyO]),
							shortcut: None,
							action: MenuEntry::no_action(),
							icon: None,
							children: MenuEntryGroups(vec![
								vec![
									MenuEntry {
										label: "Reopen Last Closed".into(),
										// shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyT]),
										..MenuEntry::default()
									},
									MenuEntry {
										label: "Clear Recently Opened".into(),
										..MenuEntry::default()
									},
								],
								vec![
									MenuEntry {
										label: "Some Recent File.gdd".into(),
										..MenuEntry::default()
									},
									MenuEntry {
										label: "Another Recent File.gdd".into(),
										..MenuEntry::default()
									},
									MenuEntry {
										label: "An Older File.gdd".into(),
										..MenuEntry::default()
									},
									MenuEntry {
										label: "Some Other Older File.gdd".into(),
										..MenuEntry::default()
									},
									MenuEntry {
										label: "Yet Another Older File.gdd".into(),
										..MenuEntry::default()
									},
								],
							]),
						},
					],
					vec![
						MenuEntry {
							label: "Close".into(),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyW]),
							shortcut: action_shortcut!(PortfolioMessageDiscriminant::CloseActiveDocumentWithConfirmation),
							action: MenuEntry::create_action(|_| PortfolioMessage::CloseActiveDocumentWithConfirmation.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Close All".into(),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyAlt, Key::KeyW]),
							shortcut: action_shortcut!(DialogMessageDiscriminant::CloseAllDocumentsWithConfirmation),
							action: MenuEntry::create_action(|_| DialogMessage::CloseAllDocumentsWithConfirmation.into()),
							..MenuEntry::default()
						},
					],
					vec![
						MenuEntry {
							label: "Save".into(),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyS]),
							shortcut: action_shortcut!(DocumentMessageDiscriminant::SaveDocument),
							action: MenuEntry::create_action(|_| DocumentMessage::SaveDocument.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Save As…".into(),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyS]),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Save All".into(),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyAlt, Key::KeyS]),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Auto-Save".into(),
							icon: Some("CheckboxChecked".into()),
							..MenuEntry::default()
						},
					],
					vec![
						MenuEntry {
							label: "Import…".into(),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyI]),
							shortcut: action_shortcut!(PortfolioMessageDiscriminant::Import),
							action: MenuEntry::create_action(|_| PortfolioMessage::Import.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Export…".into(),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyE]),
							shortcut: action_shortcut!(DialogMessageDiscriminant::RequestExportDialog),
							action: MenuEntry::create_action(|_| DialogMessage::RequestExportDialog.into()),
							..MenuEntry::default()
						},
					],
					vec![MenuEntry {
						label: "Quit".into(),
						// shortcut: Some(vec![Key::KeyControl, Key::KeyQ]),
						..MenuEntry::default()
					}],
				]),
			},
			MenuColumn {
				label: "Edit".into(),
				children: MenuEntryGroups(vec![
					vec![
						MenuEntry {
							label: "Undo".into(),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyZ]),
							shortcut: action_shortcut!(DocumentMessageDiscriminant::Undo),
							action: MenuEntry::create_action(|_| DocumentMessage::Undo.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Redo".into(),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyZ]),
							shortcut: action_shortcut!(DocumentMessageDiscriminant::Redo),
							action: MenuEntry::create_action(|_| DocumentMessage::Redo.into()),
							..MenuEntry::default()
						},
					],
					vec![
						MenuEntry {
							label: "Cut".into(),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyX]),
							shortcut: action_shortcut!(PortfolioMessageDiscriminant::Cut),
							action: MenuEntry::create_action(|_| PortfolioMessage::Cut { clipboard: Clipboard::Device }.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Copy".into(),
							icon: Some("Copy".into()),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyC]),
							shortcut: action_shortcut!(PortfolioMessageDiscriminant::Copy),
							action: MenuEntry::create_action(|_| PortfolioMessage::Copy { clipboard: Clipboard::Device }.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Paste".into(),
							icon: Some("Paste".into()),
							// shortcut: Some(vec![Key::KeyControl, Key::KeyV]),
							shortcut: action_shortcut!(FrontendMessageDiscriminant::TriggerPaste),
							action: MenuEntry::create_action(|_| FrontendMessage::TriggerPaste.into()),
							..MenuEntry::default()
						},
					],
				]),
			},
			MenuColumn {
				label: "Layer".into(),
				children: MenuEntryGroups(vec![vec![
					MenuEntry {
						label: "Select All".into(),
						// shortcut: Some(vec![Key::KeyControl, Key::KeyA]),
						shortcut: action_shortcut!(DocumentMessageDiscriminant::SelectAllLayers),
						action: MenuEntry::create_action(|_| DocumentMessage::SelectAllLayers.into()),
						..MenuEntry::default()
					},
					MenuEntry {
						label: "Deselect All".into(),
						// shortcut: Some(vec![Key::KeyControl, Key::KeyAlt, Key::KeyA]),
						shortcut: action_shortcut!(DocumentMessageDiscriminant::DeselectAllLayers),
						action: MenuEntry::create_action(|_| DocumentMessage::DeselectAllLayers.into()),
						..MenuEntry::default()
					},
					MenuEntry {
						label: "Order".into(),
						action: MenuEntry::no_action(),
						children: MenuEntryGroups(vec![vec![
							MenuEntry {
								label: "Raise To Front".into(),
								// shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyLeftBracket]),
								shortcut: action_shortcut!(DocumentMessageDiscriminant::SelectedLayersRaiseToFront),
								action: MenuEntry::create_action(|_| DocumentMessage::SelectedLayersRaiseToFront.into()),
								..MenuEntry::default()
							},
							MenuEntry {
								label: "Raise".into(),
								// shortcut: Some(vec![Key::KeyControl, Key::KeyRightBracket]),
								shortcut: action_shortcut!(DocumentMessageDiscriminant::SelectedLayersRaise),
								action: MenuEntry::create_action(|_| DocumentMessage::SelectedLayersRaise.into()),
								..MenuEntry::default()
							},
							MenuEntry {
								label: "Lower".into(),
								// shortcut: Some(vec![Key::KeyControl, Key::KeyLeftBracket]),
								shortcut: action_shortcut!(DocumentMessageDiscriminant::SelectedLayersLower),
								action: MenuEntry::create_action(|_| DocumentMessage::SelectedLayersLower.into()),
								..MenuEntry::default()
							},
							MenuEntry {
								label: "Lower to Back".into(),
								// shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyRightBracket]),
								shortcut: action_shortcut!(DocumentMessageDiscriminant::SelectedLayersLowerToBack),
								action: MenuEntry::create_action(|_| DocumentMessage::SelectedLayersLowerToBack.into()),
								..MenuEntry::default()
							},
						]]),
						..MenuEntry::default()
					},
				]]),
			},
			MenuColumn {
				label: "Document".into(),
				children: MenuEntryGroups(vec![vec![MenuEntry {
					label: "Menu entries coming soon".into(),
					..MenuEntry::default()
				}]]),
			},
			MenuColumn {
				label: "View".into(),
				children: MenuEntryGroups(vec![vec![MenuEntry {
					label: "Show/Hide Node Graph (In Development)".into(),
					action: MenuEntry::create_action(|_| WorkspaceMessage::NodeGraphToggleVisibility.into()),
					..MenuEntry::default()
				}]]),
			},
			MenuColumn {
				label: "Help".into(),
				children: MenuEntryGroups(vec![
					vec![MenuEntry {
						label: "About Graphite".into(),
						action: MenuEntry::create_action(|_| DialogMessage::RequestAboutGraphiteDialog.into()),
						..MenuEntry::default()
					}],
					vec![
						MenuEntry {
							label: "Report a Bug".into(),
							action: MenuEntry::create_action(|_| {
								FrontendMessage::TriggerVisitLink {
									url: "https://github.com/GraphiteEditor/Graphite/issues/new".into(),
								}
								.into()
							}),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Visit on GitHub".into(),
							action: MenuEntry::create_action(|_| {
								FrontendMessage::TriggerVisitLink {
									url: "https://github.com/GraphiteEditor/Graphite".into(),
								}
								.into()
							}),
							..MenuEntry::default()
						},
					],
					vec![
						MenuEntry {
							label: "Debug: Print Messages".into(),
							action: MenuEntry::no_action(),
							children: MenuEntryGroups(vec![vec![
								MenuEntry {
									label: "Off".into(),
									// icon: Some("Checkmark".into()), // TODO: Find a way to set this icon on the active mode
									// shortcut: Some(vec![Key::KeyAlt, Key::Key0]),
									shortcut: action_shortcut!(DebugMessageDiscriminant::MessageOff),
									action: MenuEntry::create_action(|_| DebugMessage::MessageOff.into()),
									..MenuEntry::default()
								},
								MenuEntry {
									label: "Only Names".into(),
									// shortcut: Some(vec![Key::KeyAlt, Key::Key1]),
									shortcut: action_shortcut!(DebugMessageDiscriminant::MessageNames),
									action: MenuEntry::create_action(|_| DebugMessage::MessageNames.into()),
									..MenuEntry::default()
								},
								MenuEntry {
									label: "Full Contents".into(),
									// shortcut: Some(vec![Key::KeyAlt, Key::Key2]),
									shortcut: action_shortcut!(DebugMessageDiscriminant::MessageContents),
									action: MenuEntry::create_action(|_| DebugMessage::MessageContents.into()),
									..MenuEntry::default()
								},
							]]),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Debug: Print Trace Logs".into(),
							icon: Some(if let log::LevelFilter::Trace = log::max_level() { "CheckboxChecked" } else { "CheckboxUnchecked" }.into()),
							// shortcut: Some(vec![Key::KeyAlt, Key::KeyT]),
							shortcut: action_shortcut!(DebugMessageDiscriminant::ToggleTraceLogs),
							action: MenuEntry::create_action(|_| DebugMessage::ToggleTraceLogs.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Debug: Print Document".into(),
							// shortcut: Some(vec![Key::KeyAlt, Key::KeyP]),
							shortcut: action_shortcut!(DocumentMessageDiscriminant::DebugPrintDocument),
							action: MenuEntry::create_action(|_| DocumentMessage::DebugPrintDocument.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Debug: Panic (DANGER)".into(),
							action: MenuEntry::create_action(|_| panic!()),
							..MenuEntry::default()
						},
					],
				]),
			},
		]))
	}
}
