use crate::messages::prelude::*;

pub use super::workspace_types::*;

#[derive(Debug, Clone)]
pub struct WorkspaceMessageHandler {
	root: DivisionOrPanel,
}

impl Default for WorkspaceMessageHandler {
	fn default() -> Self {
		let documents = Panel::new([]);
		let properties = Panel::new([TabType::Properties]);
		let layers = Panel::new([TabType::Layers, TabType::Properties, TabType::Layers]);
		let right = Division::new(Direction::Vertical, 45., 55., properties, layers);
		Self {
			root: Division::new(Direction::Horizontal, 80., 20., documents, right).into(),
		}
	}
}

impl WorkspaceMessageHandler {
	pub fn add_tab(&mut self, tab: TabType, destination: Option<TabDestination>) {
		let Some(destination) = destination else {
			self.root.first_panel_mut().add_tab(tab, None);
			return;
		};
		if let Some(edge) = destination.edge {
			let Some(destination_wrapper) = destination.panel.get_wrapped_mut(&mut self.root) else {
				warn!("Invalid destination panel for add");
				return;
			};
			let opposite = std::mem::replace(destination_wrapper, Panel::new([]).into());
			let new_panel = Panel::new([tab]).into();
			let [start, end] = if edge.start { [new_panel, opposite] } else { [opposite, new_panel] };
			*destination_wrapper = Division::new(edge.direction, 50., 50., start, end).into();
		} else {
			let Some(destination_panel) = destination.panel.get_panel_mut(&mut self.root) else {
				warn!("Invalid destination panel for add");
				return;
			};
			destination_panel.add_tab(tab, destination.insert_index);
		}
	}
	pub fn delete_tab(&mut self, tab: TabPath) {
		let Some(panel) = tab.panel.get_panel_mut(&mut self.root) else {
			warn!("Invalid panel for delete");
			return;
		};
		panel.remove_tab(tab.tab_index);
		if panel.is_empty() {
			if let Some((parent, start)) = tab.panel.get_parent_wrapped_mut(&mut self.root) {
				parent.replace_with_child(!start);
			}
		}
	}
	pub fn move_tab(&mut self, source: TabPath, destination: TabDestination) {
		let Some(panel) = source.panel.get_panel_mut(&mut self.root) else {
			warn!("Invalid source panel for move");
			return;
		};

		// Don't bother
		if destination.panel == source.panel && panel.len() == 1 {
			return;
		}

		let tab = panel.remove_tab(source.tab_index);
		let remove_division = panel.is_empty();
		self.add_tab(tab, Some(destination));

		if remove_division {
			if let Some((parent, start)) = source.panel.get_parent_wrapped_mut(&mut self.root) {
				parent.replace_with_child(!start);
			}
		}
	}
	pub fn select_tab(&mut self, tab: TabPath) {
		let Some(panel) = tab.panel.get_panel_mut(&mut self.root) else {
			warn!("Invalid panel for select");
			return;
		};
		panel.select_tab(tab.tab_index);
	}
	pub fn resize_division(&mut self, division: PanelPath, start_size: f64, end_size: f64) {
		let Some(division) = division.get_wrapped_mut(&mut self.root).and_then(|wrapped| wrapped.as_division_mut()) else {
			warn!("Invalid division for resize");
			return;
		};
		division.resize(start_size, end_size);
	}
	pub fn is_single_tab(&self, panel: PanelPath) -> bool {
		let Some(panel) = panel.get_panel(&self.root) else {
			warn!("Invalid panel for check single tab");
			return true;
		};
		panel.len() == 1
	}

	pub fn send_layout(&self, portfolio: &PortfolioMessageHandler, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateDockspace {
			root: FrontendDivisionOrPanel::new(&self.root, portfolio, PanelPath::builder()),
		});
	}
}

impl MessageHandler<WorkspaceMessage, &PortfolioMessageHandler> for WorkspaceMessageHandler {
	fn process_message(&mut self, message: WorkspaceMessage, responses: &mut VecDeque<Message>, portfolio: &PortfolioMessageHandler) {
		match message {
			WorkspaceMessage::AddTab { tab, destination } => self.add_tab(tab, destination),
			WorkspaceMessage::DeleteTab { tab } => self.delete_tab(tab),
			WorkspaceMessage::MoveTab { source, destination } => self.move_tab(source, destination),
			WorkspaceMessage::SelectTab { tab } => self.select_tab(tab),
			WorkspaceMessage::ResizeDivision { division, start_size, end_size } => self.resize_division(division, start_size, end_size),

			WorkspaceMessage::SendLayout => {} // Send layout is called anyway below.
		}
		self.send_layout(portfolio, responses);
	}

	fn actions(&self) -> ActionList {
		actions!(WorkspaceMessageDiscriminant;)
	}
}

#[cfg(test)]
const fn test_tab(identifier: u64) -> TabType {
	use crate::messages::portfolio::document::utility_types::misc::DocumentViewId;

	TabType::document(DocumentViewId(identifier), DocumentId(identifier))
}

#[cfg(test)]
fn test_layout() -> WorkspaceMessageHandler {
	let documents = Panel::new([]);
	let end_start = Panel::new([test_tab(1_000)]);
	let end_end = Panel::new([test_tab(11_000), test_tab(11_001)]);
	let right = Division::new(Direction::Vertical, 45., 55., end_start, end_end);
	WorkspaceMessageHandler {
		root: Division::new(Direction::Horizontal, 80., 20., documents, right).into(),
	}
}

#[cfg(test)]
const TEST_TAB: TabType = test_tab(123);

#[test]
fn dockspace_tab_paths() {
	let mut workspace = test_layout();
	assert!(PanelPath::builder().get_panel(&workspace.root).is_none());
	assert!(PanelPath::builder().start().start().get_panel(&workspace.root).is_none());
	assert!(PanelPath::builder().end().get_panel(&workspace.root).is_none());
	assert!(PanelPath::builder().end().end().start().get_panel(&workspace.root).is_none());
	assert_eq!(PanelPath::builder().start().get_panel(&workspace.root).unwrap().is_empty(), true);
	assert_eq!(PanelPath::builder().end().end().get_panel(&workspace.root).unwrap().len(), 2);
	let right_panel = PanelPath::builder().end().end().get_parent_wrapped_mut(&mut workspace.root).unwrap();
	assert_eq!(right_panel.0.as_division().unwrap().direction(), Direction::Vertical);
	assert_eq!(right_panel.1, false);
	let first_panel = PanelPath::builder().start().get_parent_wrapped_mut(&mut workspace.root).unwrap();
	assert_eq!(first_panel.0.as_division().unwrap().direction(), Direction::Horizontal);
	assert_eq!(first_panel.1, true);
	let empty_panel = PanelPath::builder().start().get_wrapped_mut(&mut workspace.root).unwrap();
	assert!(empty_panel.as_panel().unwrap().is_empty());
	assert!(PanelPath::builder().start().start().get_wrapped_mut(&mut workspace.root).is_none());
}

#[test]
fn dockspace_add_tab() {
	let mut workspace = test_layout();
	let destination = TabDestination {
		panel: PanelPath::builder().end().start(),
		insert_index: None,
		edge: None,
	};
	workspace.add_tab(TEST_TAB, Some(destination));
	let panel = PanelPath::builder().end().start().get_panel(&workspace.root).unwrap();
	assert_eq!(panel.len(), 2);
	assert_eq!(panel.active_index(), 1);
	assert_eq!(panel.active_tab().unwrap(), TEST_TAB);
}

#[test]
fn dockspace_insert_tab() {
	let mut workspace = test_layout();
	let destination = TabDestination {
		panel: PanelPath::builder().end().end(),
		insert_index: Some(1),
		edge: None,
	};
	workspace.add_tab(TEST_TAB, Some(destination));
	let panel = PanelPath::builder().end().end().get_panel(&workspace.root).unwrap();
	assert_eq!(panel.len(), 3);
	assert_eq!(panel.active_index(), 1);
	assert_eq!(panel.active_tab().unwrap(), TEST_TAB);
}

#[test]
fn dockspace_insert_tab_empty() {
	let mut workspace = test_layout();
	let destination = TabDestination {
		panel: PanelPath::builder().start(),
		insert_index: Some(0),
		edge: None,
	};
	workspace.add_tab(TEST_TAB, Some(destination));
	let panel = workspace.root.as_division().unwrap().start().as_panel().unwrap();
	assert_eq!(panel.len(), 1);
	assert_eq!(panel.active_index(), 0);
	assert_eq!(panel.active_tab().unwrap(), TEST_TAB);
}

#[test]
fn dockspace_add_tab_automatic() {
	let mut workspace = test_layout();
	workspace.add_tab(TEST_TAB, None);
	let panel = PanelPath::builder().start().get_panel(&workspace.root).unwrap();
	assert_eq!(panel.len(), 1);
	assert_eq!(panel.active_index(), 0);
	assert_eq!(panel.active_tab().unwrap(), TEST_TAB);
}

#[test]
fn dockspace_insert_tab_edge() {
	for (direction, start) in [(Direction::Horizontal, true), (Direction::Horizontal, false), (Direction::Vertical, true), (Direction::Vertical, false)] {
		let mut workspace = test_layout();
		let destination = TabDestination {
			panel: PanelPath::builder().start(),
			insert_index: None,
			edge: Some(InsertEdge { direction, start }),
		};
		workspace.add_tab(TEST_TAB, Some(destination));
		let division = PanelPath::builder().start().get_wrapped_mut(&mut workspace.root).unwrap().as_division().unwrap();
		assert_eq!(division.direction(), direction);
		let [insert, other] = if start { [division.start(), division.end()] } else { [division.end(), division.start()] };
		assert_eq!(other.as_panel().unwrap().len(), 0);
		assert_eq!(insert.as_panel().unwrap().active_tab().unwrap(), TEST_TAB);
	}
}

#[test]
fn dockspace_delete_tab() {
	let mut workspace = test_layout();
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 1));
	let panel = PanelPath::builder().end().end().get_panel(&workspace.root).unwrap();
	assert_eq!(panel.len(), 1);
	assert_eq!(panel.active_tab().unwrap(), test_tab(11_000));
}

#[test]
fn dockspace_delete_replace_active() {
	let mut workspace = test_layout();
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 0));
	let panel = PanelPath::builder().end().end().get_panel(&workspace.root).unwrap();
	assert_eq!(panel.len(), 1);
	assert_eq!(panel.active_tab().unwrap(), test_tab(11_001));
}

#[test]
fn dockspace_delete_division() {
	let mut workspace = test_layout();
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 0));
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 0));
	let panel = PanelPath::builder().end().get_panel(&workspace.root).unwrap();
	assert_eq!(panel.len(), 1);
	assert_eq!(panel.active_tab().unwrap(), test_tab(1_000));
}

#[test]
fn dockspace_delete_2_divisions() {
	let mut workspace = test_layout();
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 0));
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 0));
	workspace.delete_tab(TabPath::new(PanelPath::builder().end(), 0));
	let panel = PanelPath::builder().get_panel(&workspace.root).unwrap();
	assert_eq!(panel.len(), 0);
	assert!(panel.active_tab().is_none());
}

#[test]
fn dockspace_select_tab() {
	let mut workspace = test_layout();
	workspace.select_tab(TabPath::new(PanelPath::builder().end().end(), 1));
	let panel = PanelPath::builder().end().end().get_panel(&workspace.root).unwrap();
	assert_eq!(panel.active_tab().unwrap(), test_tab(11_001));
}

#[test]
fn dockspace_move_tab_simple() {
	let mut workspace = test_layout();
	let destination = TabDestination {
		panel: PanelPath::builder().end().start(),
		insert_index: None,
		edge: None,
	};
	workspace.move_tab(TabPath::new(PanelPath::builder().end().end(), 1), destination);
	let panel = PanelPath::builder().end().end().get_panel(&workspace.root).unwrap();
	assert_eq!(panel.len(), 1);
	let panel = PanelPath::builder().end().start().get_panel(&workspace.root).unwrap();
	assert_eq!(panel.len(), 2);
	assert_eq!(panel.active_tab().unwrap(), test_tab(11_001));
}

#[test]
fn dockspace_move_tab_on_self() {
	let mut workspace = test_layout();
	let destination = TabDestination {
		panel: PanelPath::builder().end().start(),
		insert_index: None,
		edge: None,
	};
	workspace.move_tab(TabPath::new(PanelPath::builder().end().start(), 0), destination);
	let panel = PanelPath::builder().end().start().get_panel_mut(&mut workspace.root).unwrap();
	assert_eq!(panel.len(), 1);
	assert_eq!(panel.active_tab().unwrap(), test_tab(1_000));
}

#[test]
fn dockspace_move_tab_on_self_stack() {
	let mut workspace = test_layout();
	let destination = TabDestination {
		panel: PanelPath::builder().end().end(),
		insert_index: None,
		edge: None,
	};
	workspace.move_tab(TabPath::new(PanelPath::builder().end().end(), 0), destination);
	let panel = PanelPath::builder().end().end().get_panel_mut(&mut workspace.root).unwrap();
	assert_eq!(panel.len(), 2);
	assert_eq!(panel.active_index(), 1);
	assert_eq!(panel.active_tab().unwrap(), test_tab(11_000));
}

#[test]
fn dockspace_move_tab_delete_divison() {
	let mut workspace = test_layout();
	let destination = TabDestination {
		panel: PanelPath::builder().end().end(),
		insert_index: Some(1),
		edge: None,
	};
	workspace.move_tab(TabPath::new(PanelPath::builder().end().start(), 0), destination);
	let panel = workspace.root.as_division().unwrap().end().as_panel().unwrap();
	assert_eq!(panel.len(), 3);
	assert_eq!(panel.active_index(), 1);
	assert_eq!(panel.active_tab().unwrap(), test_tab(1_000));
}

#[test]
fn dockspace_move_tab_edge() {
	for (direction, start) in [(Direction::Horizontal, true), (Direction::Horizontal, false), (Direction::Vertical, true), (Direction::Vertical, false)] {
		let mut workspace = test_layout();
		let destination = TabDestination {
			panel: PanelPath::builder().end().start(),
			insert_index: None,
			edge: Some(InsertEdge { direction, start }),
		};
		workspace.move_tab(TabPath::new(PanelPath::builder().end().end(), 0), destination);
		let division = workspace.root.as_division().unwrap().end().as_division().unwrap().start().as_division().unwrap();
		assert_eq!(division.direction(), direction);
		let start_panel = division.start().as_panel().unwrap();
		let end_panel = division.end().as_panel().unwrap();
		assert_eq!(start_panel.len(), 1);
		assert_eq!(end_panel.len(), 1);
		assert_eq!(start_panel.active_tab().unwrap(), test_tab(if start { 11_000 } else { 1_000 }));
		assert_eq!(end_panel.active_tab().unwrap(), test_tab(if start { 1_000 } else { 11_000 }));
	}
}

#[test]
fn dockspace_move_tab_edge_delete_other() {
	for (direction, start) in [(Direction::Horizontal, true), (Direction::Horizontal, false), (Direction::Vertical, true), (Direction::Vertical, false)] {
		let mut workspace = test_layout();
		let destination = TabDestination {
			panel: PanelPath::builder().end().end(),
			insert_index: None,
			edge: Some(InsertEdge { direction, start }),
		};
		workspace.move_tab(TabPath::new(PanelPath::builder().end().start(), 0), destination);
		let division = workspace.root.as_division().unwrap().end().as_division().unwrap();
		assert_eq!(division.direction(), direction);

		let [insert, rest] = if start { [division.start(), division.end()] } else { [division.end(), division.start()] };
		let insert_panel = insert.as_panel().unwrap();
		let rest_panel = rest.as_panel().unwrap();
		assert_eq!(insert_panel.len(), 1);
		assert_eq!(rest_panel.len(), 2);
		assert_eq!(insert_panel.active_tab().unwrap(), test_tab(1_000));
		assert_eq!(rest_panel.active_tab().unwrap(), test_tab(11_000));
	}
}

#[test]
fn dockspace_can_not_edge_self() {
	for (direction, start) in [(Direction::Horizontal, true), (Direction::Horizontal, false), (Direction::Vertical, true), (Direction::Vertical, false)] {
		let mut workspace = test_layout();
		let destination = TabDestination {
			panel: PanelPath::builder().end().start(),
			insert_index: None,
			edge: Some(InsertEdge { direction, start }),
		};
		workspace.move_tab(TabPath::new(PanelPath::builder().end().start(), 0), destination);
		let panel = workspace.root.as_division().unwrap().end().as_division().unwrap().start().as_panel().unwrap();
		assert_eq!(panel.len(), 1);
		assert_eq!(panel.active_tab().unwrap(), test_tab(1_000));
	}
}

#[test]
fn dockspace_can_edge_self_with_other_tabs() {
	for (direction, start) in [(Direction::Horizontal, true), (Direction::Horizontal, false), (Direction::Vertical, true), (Direction::Vertical, false)] {
		let mut workspace = test_layout();
		let destination = TabDestination {
			panel: PanelPath::builder().end().end(),
			insert_index: None,
			edge: Some(InsertEdge { direction, start }),
		};
		workspace.move_tab(TabPath::new(PanelPath::builder().end().end(), 0), destination);
		let division = workspace.root.as_division().unwrap().end().as_division().unwrap().end().as_division().unwrap();
		let [moved, other] = if start { [division.start(), division.end()] } else { [division.end(), division.start()] };
		assert_eq!(moved.as_panel().unwrap().active_tab().unwrap(), test_tab(11_000));
		assert_eq!(other.as_panel().unwrap().active_tab().unwrap(), test_tab(11_001));
	}
}

#[test]
fn dockspace_resize() {
	let mut workspace = test_layout();

	workspace.resize_division(PanelPath::builder().end(), 11., 13.);
	let division = workspace.root.as_division().unwrap().end().as_division().unwrap();
	assert_eq!(division.size(), [11., 13.])
}
