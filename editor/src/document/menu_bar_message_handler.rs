use super::MenuBarMessage;

use crate::input::keyboard::Key;
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

impl PropertyHolder for MenuBarMessageHandler {
	fn properties(&self) -> Layout {
		Layout::MenuLayout(MenuLayout::new(vec![
			MenuColumn {
				label: "File".into(),
				children: vec![
					vec![
						MenuEntry {
							label: "New…".into(),
							icon: Some("File".into()),
							action: MenuEntry::create_action(|_| DialogMessage::RequestNewDocumentDialog.into()),
							shortcut: Some(vec![Key::KeyControl, Key::KeyN]),
							children: None,
						},
						MenuEntry {
							label: "Open…".into(),
							shortcut: Some(vec![Key::KeyControl, Key::KeyO]),
							action: MenuEntry::create_action(|_| PortfolioMessage::OpenDocument.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Open Recent".into(),
							shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyO]),
							action: MenuEntry::no_action(),
							icon: None,
							children: Some(vec![
								vec![
									MenuEntry {
										label: "Reopen Last Closed".into(),
										shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyT]),
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
							shortcut: Some(vec![Key::KeyControl, Key::KeyW]),
							action: MenuEntry::create_action(|_| PortfolioMessage::CloseActiveDocumentWithConfirmation.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Close All".into(),
							shortcut: Some(vec![Key::KeyControl, Key::KeyAlt, Key::KeyW]),
							action: MenuEntry::create_action(|_| DialogMessage::CloseAllDocumentsWithConfirmation.into()),
							..MenuEntry::default()
						},
					],
					vec![
						MenuEntry {
							label: "Save".into(),
							shortcut: Some(vec![Key::KeyControl, Key::KeyS]),
							action: MenuEntry::create_action(|_| DocumentMessage::SaveDocument.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Save As…".into(),
							shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyS]),
							action: MenuEntry::create_action(|_| DocumentMessage::SaveDocument.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Save All".into(),
							shortcut: Some(vec![Key::KeyControl, Key::KeyAlt, Key::KeyS]),
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
							shortcut: Some(vec![Key::KeyControl, Key::KeyI]),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Export…".into(),
							shortcut: Some(vec![Key::KeyControl, Key::KeyE]),
							action: MenuEntry::create_action(|_| DialogMessage::RequestExportDialog.into()),
							..MenuEntry::default()
						},
					],
					vec![MenuEntry {
						label: "Quit".into(),
						shortcut: Some(vec![Key::KeyControl, Key::KeyQ]),
						..MenuEntry::default()
					}],
				],
			},
			MenuColumn {
				label: "Edit".into(),
				children: vec![
					vec![
						MenuEntry {
							label: "Undo".into(),
							shortcut: Some(vec![Key::KeyControl, Key::KeyZ]),
							action: MenuEntry::create_action(|_| DocumentMessage::Undo.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Redo".into(),
							shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyZ]),
							action: MenuEntry::create_action(|_| DocumentMessage::Redo.into()),
							..MenuEntry::default()
						},
					],
					vec![
						MenuEntry {
							label: "Cut".into(),
							shortcut: Some(vec![Key::KeyControl, Key::KeyX]),
							action: MenuEntry::create_action(|_| PortfolioMessage::Cut { clipboard: Clipboard::Device }.into()),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Copy".into(),
							icon: Some("Copy".into()),
							shortcut: Some(vec![Key::KeyControl, Key::KeyC]),
							action: MenuEntry::create_action(|_| PortfolioMessage::Copy { clipboard: Clipboard::Device }.into()),
							..MenuEntry::default()
						},
						// TODO: Fix this
						// { label: "Paste", icon: "Paste", shortcut: ["KeyControl", "KeyV"], action: async (): Promise<void> => editor.instance.paste() },
					],
				],
			},
			MenuColumn {
				label: "Layer".into(),
				children: vec![vec![
					MenuEntry {
						label: "Select All".into(),
						shortcut: Some(vec![Key::KeyControl, Key::KeyA]),
						action: MenuEntry::create_action(|_| DocumentMessage::SelectAllLayers.into()),
						..MenuEntry::default()
					},
					MenuEntry {
						label: "Deselect All".into(),
						shortcut: Some(vec![Key::KeyControl, Key::KeyAlt, Key::KeyA]),
						action: MenuEntry::create_action(|_| DocumentMessage::DeselectAllLayers.into()),
						..MenuEntry::default()
					},
					MenuEntry {
						label: "Order".into(),
						action: MenuEntry::no_action(),
						children: Some(vec![vec![
							MenuEntry {
								label: "Raise To Front".into(),
								shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyLeftBracket]),
								action: MenuEntry::create_action(|_| DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MAX }.into()),
								..MenuEntry::default()
							},
							MenuEntry {
								label: "Raise".into(),
								shortcut: Some(vec![Key::KeyControl, Key::KeyRightBracket]),
								action: MenuEntry::create_action(|_| DocumentMessage::ReorderSelectedLayers { relative_index_offset: 1 }.into()),
								..MenuEntry::default()
							},
							MenuEntry {
								label: "Lower".into(),
								shortcut: Some(vec![Key::KeyControl, Key::KeyLeftBracket]),
								action: MenuEntry::create_action(|_| DocumentMessage::ReorderSelectedLayers { relative_index_offset: -1 }.into()),
								..MenuEntry::default()
							},
							MenuEntry {
								label: "Lower to Back".into(),
								shortcut: Some(vec![Key::KeyControl, Key::KeyShift, Key::KeyRightBracket]),
								action: MenuEntry::create_action(|_| DocumentMessage::ReorderSelectedLayers { relative_index_offset: isize::MIN }.into()),
								..MenuEntry::default()
							},
						]]),
						..MenuEntry::default()
					},
				]],
			},
			MenuColumn {
				label: "Document".into(),
				children: vec![vec![MenuEntry {
					label: "Menu entries coming soon".into(),
					..MenuEntry::default()
				}]],
			},
			MenuColumn {
				label: "View".into(),
				children: vec![vec![MenuEntry {
					label: "Show/Hide Node Graph (In Development)".into(),
					action: MenuEntry::create_action(|_| WorkspaceMessage::NodeGraphToggleVisibility.into()),
					..MenuEntry::default()
				}]],
			},
			MenuColumn {
				label: "Help".into(),
				children: vec![
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
							label: "Debug: Set Log Level".into(),
							action: MenuEntry::no_action(),
							children: Some(vec![vec![
								MenuEntry {
									label: "Log Level Info".into(),
									action: MenuEntry::create_action(|_| GlobalMessage::LogInfo.into()),
									shortcut: Some(vec![Key::Key1]),
									..MenuEntry::default()
								},
								MenuEntry {
									label: "Log Level Debug".into(),
									action: MenuEntry::create_action(|_| GlobalMessage::LogDebug.into()),
									shortcut: Some(vec![Key::Key2]),
									..MenuEntry::default()
								},
								MenuEntry {
									label: "Log Level Trace".into(),
									action: MenuEntry::create_action(|_| GlobalMessage::LogTrace.into()),
									shortcut: Some(vec![Key::Key3]),
									..MenuEntry::default()
								},
							]]),
							..MenuEntry::default()
						},
						MenuEntry {
							label: "Debug: Panic (DANGER)".into(),
							action: MenuEntry::create_action(|_| panic!()),
							..MenuEntry::default()
						},
					],
				],
			},
		]))
	}
}
