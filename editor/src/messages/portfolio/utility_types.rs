use graphene_std::Color;
use graphene_std::raster::Image;
use graphene_std::text::{Font, FontCache};

#[derive(Debug, Default)]
pub struct CachedData {
	pub font_cache: FontCache,
	pub font_catalog: FontCatalog,
}

// TODO: Should this be a BTreeMap instead?
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FontCatalog(pub Vec<FontCatalogFamily>);

impl FontCatalog {
	pub fn find_font_style_in_catalog(&self, font: &Font) -> Option<FontCatalogStyle> {
		let family = self.0.iter().find(|family| family.name == font.font_family);

		let found_style = family.map(|family| {
			let FontCatalogStyle { weight, italic, .. } = FontCatalogStyle::from_named_style(&font.font_style, "");
			family.closest_style(weight, italic).clone()
		});

		if found_style.is_none() {
			log::warn!("Font not found in catalog: {:?}", font);
		}

		found_style
	}
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FontCatalogFamily {
	/// The font family name.
	pub name: String,
	/// The font styles (variants) available for the font family.
	pub styles: Vec<FontCatalogStyle>,
}

impl FontCatalogFamily {
	/// Finds the closest style to the given weight and italic setting.
	/// Aims to find the nearest weight while maintaining the italic setting if possible, but italic may change if no other option is available.
	pub fn closest_style(&self, weight: u32, italic: bool) -> &FontCatalogStyle {
		self.styles
			.iter()
			.map(|style| ((style.weight as i32 - weight as i32).unsigned_abs() + 10000 * (style.italic != italic) as u32, style))
			.min_by_key(|(distance, _)| *distance)
			.map(|(_, style)| style)
			.unwrap_or(&self.styles[0])
	}
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FontCatalogStyle {
	pub weight: u32,
	pub italic: bool,
	pub url: String,
}

impl FontCatalogStyle {
	pub fn to_named_style(&self) -> String {
		let weight = self.weight;
		let italic = self.italic;

		let named_weight = Font::named_weight(weight);
		let maybe_italic = if italic { " Italic" } else { "" };

		format!("{named_weight}{maybe_italic} ({weight})")
	}

	pub fn from_named_style(named_style: &str, url: impl Into<String>) -> FontCatalogStyle {
		let weight = named_style.split_terminator(['(', ')']).next_back().and_then(|x| x.parse::<u32>().ok()).unwrap_or(400);
		let italic = named_style.contains("Italic (");
		FontCatalogStyle { weight, italic, url: url.into() }
	}

	/// Get the URL for the stylesheet for loading a font preview for this style of the given family name, subsetted to only the letters in the family name.
	pub fn preview_url(&self, family: impl Into<String>) -> String {
		let name = family.into().replace(' ', "+");
		let italic = if self.italic { "ital," } else { "" };
		let weight = self.weight;
		format!("https://fonts.googleapis.com/css2?display=swap&family={name}:{italic}wght@{weight}&text={name}")
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum PanelType {
	Welcome,
	Document,
	Layers,
	Properties,
	Data,
}

impl From<String> for PanelType {
	fn from(value: String) -> Self {
		match value.as_str() {
			"Welcome" => PanelType::Welcome,
			"Document" => PanelType::Document,
			"Layers" => PanelType::Layers,
			"Properties" => PanelType::Properties,
			"Data" => PanelType::Data,
			_ => panic!("Unknown panel type: {value}"),
		}
	}
}

impl PanelType {
	pub fn non_document_panels() -> &'static [PanelType] {
		&[PanelType::Layers, PanelType::Properties, PanelType::Data]
	}
}

/// Unique identifier for a panel group (a leaf subdivision in the layout tree that holds tabs).
#[repr(transparent)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PanelGroupId(pub u64);

/// Which edge of a panel group to split on when docking a dragged panel.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DockingSplitDirection {
	Left,
	Right,
	Top,
	Bottom,
}

/// State of a single panel group (leaf subdivision) in the workspace layout tree.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PanelGroupState {
	pub tabs: Vec<PanelType>,
	pub active_tab_index: usize,
}

impl PanelGroupState {
	pub fn active_panel_type(&self) -> Option<PanelType> {
		self.tabs.get(self.active_tab_index).copied()
	}

	pub fn contains(&self, panel_type: PanelType) -> bool {
		self.tabs.contains(&panel_type)
	}

	pub fn is_visible(&self, panel_type: PanelType) -> bool {
		self.active_panel_type() == Some(panel_type)
	}
}

/// A subdivision in the workspace layout tree. The root is always a row (horizontal).
/// Direction alternates at each depth: row, column, row, column, etc.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PanelLayoutSubdivision {
	/// A leaf subdivision: a panel group with tabbed panels.
	PanelGroup { id: PanelGroupId, state: PanelGroupState },
	/// A container subdivision that splits its space among children. Direction is implicit from depth (even = row, odd = column).
	Split { children: Vec<SplitChild> },
}

impl Default for PanelLayoutSubdivision {
	fn default() -> Self {
		PanelLayoutSubdivision::Split { children: Vec::new() }
	}
}

/// A child within a split container, with a proportional size weight.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SplitChild {
	pub subdivision: PanelLayoutSubdivision,
	/// Flex-grow weight for proportional sizing.
	pub size: f64,
}

/// The complete workspace panel layout as a tree of nested rows and columns.
/// The root subdivision is always a row (horizontal split). Direction alternates at each depth.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspacePanelLayout {
	#[serde(default)]
	pub root: PanelLayoutSubdivision,
	/// Counter for generating unique panel group IDs.
	#[serde(default)]
	next_group_id: PanelGroupId,
	/// Remembers where a panel was before being removed (panel type, group ID, and tab index), so it can be restored there.
	#[serde(default)]
	saved_positions: Vec<(PanelType, PanelGroupId, usize)>,
	/// Whether Focus Document mode is active, hiding all non-document panels.
	#[serde(default)]
	pub focus_document: bool,
}

impl WorkspacePanelLayout {
	/// Generate a new unique panel group ID.
	pub fn next_id(&mut self) -> PanelGroupId {
		let id = self.next_group_id;
		self.next_group_id.0 += 1;
		id
	}

	/// Find the panel group state for a given ID.
	pub fn panel_group(&self, id: PanelGroupId) -> Option<&PanelGroupState> {
		self.root.find_group(id)
	}

	/// Find the panel group state for a given ID (mutable).
	pub fn panel_group_mut(&mut self, id: PanelGroupId) -> Option<&mut PanelGroupState> {
		self.root.find_group_mut(id)
	}

	/// Find which panel group contains a given panel type, returning its ID.
	pub fn find_panel(&self, panel_type: PanelType) -> Option<PanelGroupId> {
		self.root.find_panel(panel_type)
	}

	/// Check if a panel type is the active (visible) tab in any panel group.
	pub fn is_panel_visible(&self, panel_type: PanelType) -> bool {
		self.find_panel(panel_type).and_then(|id| self.panel_group(id)).is_some_and(|group| group.is_visible(panel_type))
	}

	/// Check if a panel type is present (as any tab) in any panel group, whether or not it's the active tab.
	pub fn is_panel_present(&self, panel_type: PanelType) -> bool {
		self.find_panel(panel_type).is_some()
	}

	/// Remove empty panel groups and collapse unnecessary single-child splits.
	pub fn prune(&mut self) {
		self.root.prune();
	}

	/// Produce a filtered copy of this layout containing only the document panel, for use in Focus Document mode.
	pub fn document_only_layout(&self) -> WorkspacePanelLayout {
		let mut layout = self.clone();
		layout.root.retain_only_document_panels();
		layout.root.prune();
		layout
	}

	/// Split a panel group by inserting a new panel group adjacent to it.
	/// The direction determines where the new group goes relative to the target.
	/// Left/Right creates a horizontal (row) split, Top/Bottom creates a vertical (column) split.
	/// Returns the ID of the newly created panel group, or `None` if insertion failed.
	pub fn split_panel_group(&mut self, target_group_id: PanelGroupId, direction: DockingSplitDirection, tabs: Vec<PanelType>, active_tab_index: usize) -> Option<PanelGroupId> {
		let new_id = self.next_id();
		let new_group = SplitChild {
			subdivision: PanelLayoutSubdivision::PanelGroup {
				id: new_id,
				state: PanelGroupState { tabs, active_tab_index },
			},
			size: 50.,
		};

		let insert_before = matches!(direction, DockingSplitDirection::Left | DockingSplitDirection::Top);
		let needs_horizontal = matches!(direction, DockingSplitDirection::Left | DockingSplitDirection::Right);

		self.root.insert_split_adjacent(target_group_id, new_group, insert_before, needs_horizontal, 0).then_some(new_id)
	}

	/// Recalculate the default sizes for all splits in the tree based on document panel proximity.
	pub fn recalculate_default_sizes(&mut self) {
		self.root.recalculate_default_sizes();
	}

	/// Remember which panel group and tab index a panel was in before removal, so it can be restored there later.
	pub fn save_panel_position(&mut self, panel_type: PanelType) {
		if let Some(group_id) = self.find_panel(panel_type) {
			let tab_index = self.panel_group(group_id).and_then(|g| g.tabs.iter().position(|&t| t == panel_type)).unwrap_or(0);

			// Replace any existing saved position for this panel type
			self.saved_positions.retain(|(pt, _, _)| *pt != panel_type);
			self.saved_positions.push((panel_type, group_id, tab_index));
		}
	}

	/// Restore a panel to its previous position if available, otherwise to its default position.
	/// If the panel was previously in a group that still exists, it's added back as a tab at its original index.
	/// Otherwise, it's placed at its default structural position in the tree.
	pub fn restore_panel(&mut self, panel_type: PanelType) {
		// Try to restore to the previously saved group and tab position
		let saved = self.saved_positions.iter().find(|(pt, _, _)| *pt == panel_type).copied();
		if let Some((_, saved_group_id, saved_tab_index)) = saved
			&& let Some(group) = self.panel_group_mut(saved_group_id)
		{
			let insert_index = saved_tab_index.min(group.tabs.len());
			group.tabs.insert(insert_index, panel_type);
			group.active_tab_index = insert_index;
			self.saved_positions.retain(|(pt, _, _)| *pt != panel_type);
			return;
		}
		self.saved_positions.retain(|(pt, _, _)| *pt != panel_type);

		self.restore_panel_to_default_position(panel_type);
	}

	/// Place a panel at its default structural position in the layout tree.
	/// - Data: below the document in the left column (root child 0)
	/// - Properties: top of the right column (root child 1)
	/// - Layers: bottom of the right column (root child 1)
	fn restore_panel_to_default_position(&mut self, panel_type: PanelType) {
		let new_id = self.next_id();
		let new_group = SplitChild {
			subdivision: PanelLayoutSubdivision::PanelGroup {
				id: new_id,
				state: PanelGroupState {
					tabs: vec![panel_type],
					active_tab_index: 0,
				},
			},
			size: match panel_type {
				PanelType::Data => 30.,
				PanelType::Properties => 45.,
				PanelType::Layers => 55.,
				_ => 50.,
			},
		};

		// Determine which root child column to insert into and at which position
		let (root_child_index, insert_at_end) = match panel_type {
			PanelType::Data => (0, true),        // Left column, after document
			PanelType::Properties => (1, false), // Right column, at top
			PanelType::Layers => (1, true),      // Right column, at bottom
			_ => (1, true),
		};

		// Ensure the root is a split
		if !matches!(&self.root, PanelLayoutSubdivision::Split { .. }) {
			let old_root = std::mem::replace(&mut self.root, PanelLayoutSubdivision::Split { children: vec![] });
			if let PanelLayoutSubdivision::Split { children } = &mut self.root {
				children.push(SplitChild { subdivision: old_root, size: 80. });
			}
		}

		let PanelLayoutSubdivision::Split { children: root_children } = &mut self.root else { return };

		// Ensure the target root child exists
		while root_children.len() <= root_child_index {
			root_children.push(SplitChild {
				subdivision: PanelLayoutSubdivision::Split { children: vec![] },
				size: 20.,
			});
		}

		// The target should be a split (column at depth 1) so we can add children to it
		let target = &mut root_children[root_child_index].subdivision;
		if !matches!(target, PanelLayoutSubdivision::Split { .. }) {
			let old_subdivision = std::mem::replace(target, PanelLayoutSubdivision::Split { children: vec![] });
			if let PanelLayoutSubdivision::Split { children } = target {
				children.push(SplitChild {
					subdivision: old_subdivision,
					size: 50.,
				});
			}
		}

		if let PanelLayoutSubdivision::Split { children } = target {
			if insert_at_end {
				children.push(new_group);
			} else {
				children.insert(0, new_group);
			}
		}
	}
}

impl Default for WorkspacePanelLayout {
	fn default() -> Self {
		// Default layout (sizes are recalculated by `recalculate_default_sizes` before being sent to the frontend):
		// Row [
		//   Column [Document]
		//   Column [Properties, Layers]
		// ]
		Self {
			root: PanelLayoutSubdivision::Split {
				children: vec![
					SplitChild {
						subdivision: PanelLayoutSubdivision::Split {
							children: vec![SplitChild {
								subdivision: PanelLayoutSubdivision::PanelGroup {
									id: PanelGroupId(0),
									state: PanelGroupState {
										tabs: vec![PanelType::Document],
										active_tab_index: 0,
									},
								},
								size: 100.,
							}],
						},
						size: 80.,
					},
					SplitChild {
						subdivision: PanelLayoutSubdivision::Split {
							children: vec![
								SplitChild {
									subdivision: PanelLayoutSubdivision::PanelGroup {
										id: PanelGroupId(1),
										state: PanelGroupState {
											tabs: vec![PanelType::Properties],
											active_tab_index: 0,
										},
									},
									size: 50.,
								},
								SplitChild {
									subdivision: PanelLayoutSubdivision::PanelGroup {
										id: PanelGroupId(2),
										state: PanelGroupState {
											tabs: vec![PanelType::Layers],
											active_tab_index: 0,
										},
									},
									size: 50.,
								},
							],
						},
						size: 20.,
					},
				],
			},
			next_group_id: PanelGroupId(3),
			saved_positions: Vec::new(),
			focus_document: false,
		}
	}
}

impl PanelLayoutSubdivision {
	/// Find the panel group state for a given ID.
	pub fn find_group(&self, target_id: PanelGroupId) -> Option<&PanelGroupState> {
		match self {
			PanelLayoutSubdivision::PanelGroup { id, state } if *id == target_id => Some(state),
			PanelLayoutSubdivision::PanelGroup { .. } => None,
			PanelLayoutSubdivision::Split { children } => children.iter().find_map(|child| child.subdivision.find_group(target_id)),
		}
	}

	/// Find the panel group state for a given ID (mutable).
	pub fn find_group_mut(&mut self, target_id: PanelGroupId) -> Option<&mut PanelGroupState> {
		match self {
			PanelLayoutSubdivision::PanelGroup { id, state } if *id == target_id => Some(state),
			PanelLayoutSubdivision::PanelGroup { .. } => None,
			PanelLayoutSubdivision::Split { children } => children.iter_mut().find_map(|child| child.subdivision.find_group_mut(target_id)),
		}
	}

	/// Find the panel group ID that contains a given panel type.
	pub fn find_panel(&self, panel_type: PanelType) -> Option<PanelGroupId> {
		match self {
			PanelLayoutSubdivision::PanelGroup { id, state } if state.contains(panel_type) => Some(*id),
			PanelLayoutSubdivision::PanelGroup { .. } => None,
			PanelLayoutSubdivision::Split { children } => children.iter().find_map(|child| child.subdivision.find_panel(panel_type)),
		}
	}

	/// Collect all panel group IDs in the tree.
	pub fn all_group_ids(&self) -> Vec<PanelGroupId> {
		match self {
			PanelLayoutSubdivision::PanelGroup { id, .. } => vec![*id],
			PanelLayoutSubdivision::Split { children } => children.iter().flat_map(|child| child.subdivision.all_group_ids()).collect(),
		}
	}

	/// Remove empty panel groups and collapse unnecessary nesting.
	/// Does NOT collapse single-child splits into their child, as that would change subdivision depths
	/// and break the direction-by-depth alternation system.
	pub fn prune(&mut self) {
		let PanelLayoutSubdivision::Split { children } = self else { return };

		// Recursively prune children
		children.iter_mut().for_each(|child| child.subdivision.prune());

		// Remove empty panel groups
		children.retain(|child| !matches!(&child.subdivision, PanelLayoutSubdivision::PanelGroup { state, .. } if state.tabs.is_empty()));

		// Remove empty splits (splits that lost all their children after pruning)
		children.retain(|child| !matches!(&child.subdivision, PanelLayoutSubdivision::Split { children } if children.is_empty()));
	}

	/// Remove all non-document/non-welcome tabs from panel groups, leaving only document-related panels.
	pub fn retain_only_document_panels(&mut self) {
		match self {
			PanelLayoutSubdivision::PanelGroup { state, .. } => {
				state.tabs.retain(|t| matches!(t, PanelType::Document | PanelType::Welcome));
				state.active_tab_index = state.active_tab_index.min(state.tabs.len().saturating_sub(1));
			}
			PanelLayoutSubdivision::Split { children } => {
				children.iter_mut().for_each(|child| child.subdivision.retain_only_document_panels());
			}
		}
	}

	/// Check if this subtree contains a panel group with the given ID.
	pub fn contains_group(&self, target_id: PanelGroupId) -> bool {
		match self {
			PanelLayoutSubdivision::PanelGroup { id, .. } => *id == target_id,
			PanelLayoutSubdivision::Split { children } => children.iter().any(|child| child.subdivision.contains_group(target_id)),
		}
	}

	/// Inserts a new split child adjacent to a target panel group and returns whether the insertion was successful.
	/// Recurses to the deepest split closest to the target that matches the requested split direction.
	/// If the target is a direct child of a mismatched-direction split, this wraps it in a new sub-split.
	pub fn insert_split_adjacent(&mut self, target_id: PanelGroupId, new_child: SplitChild, insert_before: bool, needs_horizontal: bool, depth: usize) -> bool {
		let PanelLayoutSubdivision::Split { children } = self else { return false };

		let is_horizontal = depth.is_multiple_of(2);
		let direction_matches = is_horizontal == needs_horizontal;

		// Find which child subtree contains the target
		let Some(containing_index) = children.iter().position(|child| child.subdivision.contains_group(target_id)) else {
			return false;
		};

		// If the target is a direct child: we can certainly insert the new split, either as a sibling (if direction matches) or wrapping the target in a new split (if direction is mismatched)
		let target_is_direct_child = matches!(&children[containing_index].subdivision, PanelLayoutSubdivision::PanelGroup { id, .. } if *id == target_id);
		if target_is_direct_child {
			// Direction matches and target is right here: insert as a sibling
			if direction_matches {
				let insert_index = if insert_before { containing_index } else { containing_index + 1 };
				children.insert(insert_index, new_child);
			}
			// Direction mismatch: wrap the target in a new sub-split (at depth+1, which has the opposite direction of this and thus is the requested direction)
			else {
				let old_child_subdivision = std::mem::replace(&mut children[containing_index].subdivision, PanelLayoutSubdivision::Split { children: vec![] });
				let old_child = SplitChild {
					subdivision: old_child_subdivision,
					size: 50.,
				};

				if let PanelLayoutSubdivision::Split { children: sub_children } = &mut children[containing_index].subdivision {
					if insert_before {
						sub_children.push(new_child);
						sub_children.push(old_child);
					} else {
						sub_children.push(old_child);
						sub_children.push(new_child);
					}
				}
			}

			return true;
		}

		// The target is deeper, so recurse into the containing child's subtree and return its insertion outcome
		children[containing_index]
			.subdivision
			.insert_split_adjacent(target_id, new_child.clone(), insert_before, needs_horizontal, depth + 1)
	}

	/// Check if this subtree contains the document panel.
	pub fn contains_document(&self) -> bool {
		match self {
			PanelLayoutSubdivision::PanelGroup { state, .. } => state.contains(PanelType::Document) || state.contains(PanelType::Welcome),
			PanelLayoutSubdivision::Split { children } => children.iter().any(|child| child.subdivision.contains_document()),
		}
	}

	/// Recalculate the default sizes for this subdivision's children based on proximity to the document panel.
	/// Splits directly surrounding the document panel use 80-20 weighting.
	/// All other splits use equal division.
	pub fn recalculate_default_sizes(&mut self) {
		if let PanelLayoutSubdivision::Split { children } = self {
			let child_count = children.len();
			if child_count == 0 {
				return;
			}

			// Check if any child directly contains (or is) the document panel
			let document_child_index = children.iter().position(|child| child.subdivision.contains_document());

			if let Some(document_index) = document_child_index {
				// This split directly surrounds the document panel, so use 80-20 weighting
				let non_document_count = child_count - 1;
				let document_share = if non_document_count > 0 { 80. } else { 100. };
				let other_share = if non_document_count > 0 { 20. / non_document_count as f64 } else { 0. };

				for (i, child) in children.iter_mut().enumerate() {
					child.size = if i == document_index { document_share } else { other_share };
				}
			} else {
				// This split doesn't directly contain the document, use equal division
				let equal_share = 100. / child_count as f64;
				for child in children.iter_mut() {
					child.size = equal_share;
				}
			}

			// Recurse into children
			for child in children.iter_mut() {
				child.subdivision.recalculate_default_sizes();
			}
		}
	}

	/// Remove a panel group by ID from the tree. Does not prune.
	pub fn remove_group(&mut self, target_id: PanelGroupId) {
		if let PanelLayoutSubdivision::Split { children } = self {
			children.retain(|child| !matches!(&child.subdivision, PanelLayoutSubdivision::PanelGroup { id, .. } if *id == target_id));

			children.iter_mut().for_each(|child| child.subdivision.remove_group(target_id));
		}
	}
}

pub enum FileContent {
	/// A Graphite document.
	Document(String),
	/// A bitmap image.
	Image(Image<Color>),
	/// An SVG file string.
	Svg(String),
	/// Any other unsupported/unrecognized file type.
	Unsupported,
}
