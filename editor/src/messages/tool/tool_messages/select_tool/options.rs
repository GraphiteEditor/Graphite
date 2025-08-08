use super::super::tool_prelude::*;
use super::SelectTool;
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis, GroupFolderType};
use crate::messages::tool::common_functionality::pivot::{PivotGizmoType, PivotToolSource, pin_pivot_widget, pivot_gizmo_type_widget, pivot_reference_point_widget};
use graphene_std::path_bool::BooleanOperation;
use graphene_std::vector::ReferencePoint;
use std::fmt;

#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum SelectOptionsUpdate {
	NestedSelectionBehavior(NestedSelectionBehavior),
	PivotGizmoType(PivotGizmoType),
	TogglePivotGizmoType(bool),
	TogglePivotPinned,
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum NestedSelectionBehavior {
	#[default]
	Shallowest,
	Deepest,
}

impl fmt::Display for NestedSelectionBehavior {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			NestedSelectionBehavior::Deepest => write!(f, "Deep Select"),
			NestedSelectionBehavior::Shallowest => write!(f, "Shallow Select"),
		}
	}
}

impl SelectTool {
	fn deep_selection_widget(&self) -> WidgetHolder {
		let layer_selection_behavior_entries = [NestedSelectionBehavior::Shallowest, NestedSelectionBehavior::Deepest]
			.iter()
			.map(|mode| {
				MenuListEntry::new(format!("{mode:?}"))
					.label(mode.to_string())
					.on_commit(move |_| SelectToolMessage::SelectOptions(SelectOptionsUpdate::NestedSelectionBehavior(*mode)).into())
			})
			.collect();

		DropdownInput::new(vec![layer_selection_behavior_entries])
			.selected_index(Some((self.tool_data.nested_selection_behavior == NestedSelectionBehavior::Deepest) as u32))
			.tooltip(
				"Selection Mode\n\
				\n\
				Shallow Select: clicks initially select the least-nested layers and double clicks drill deeper into the folder hierarchy.\n\
				Deep Select: clicks directly select the most-nested layers in the folder hierarchy.",
			)
			.widget_holder()
	}

	fn alignment_widgets(&self, disabled: bool) -> impl Iterator<Item = WidgetHolder> + use<> {
		[AlignAxis::X, AlignAxis::Y]
			.into_iter()
			.flat_map(|axis| [(axis, AlignAggregate::Min), (axis, AlignAggregate::Center), (axis, AlignAggregate::Max)])
			.map(move |(axis, aggregate)| {
				let (icon, tooltip) = match (axis, aggregate) {
					(AlignAxis::X, AlignAggregate::Min) => ("AlignLeft", "Align Left"),
					(AlignAxis::X, AlignAggregate::Center) => ("AlignHorizontalCenter", "Align Horizontal Center"),
					(AlignAxis::X, AlignAggregate::Max) => ("AlignRight", "Align Right"),
					(AlignAxis::Y, AlignAggregate::Min) => ("AlignTop", "Align Top"),
					(AlignAxis::Y, AlignAggregate::Center) => ("AlignVerticalCenter", "Align Vertical Center"),
					(AlignAxis::Y, AlignAggregate::Max) => ("AlignBottom", "Align Bottom"),
				};
				IconButton::new(icon, 24)
					.tooltip(tooltip)
					.on_update(move |_| DocumentMessage::AlignSelectedLayers { axis, aggregate }.into())
					.disabled(disabled)
					.widget_holder()
			})
	}

	fn flip_widgets(&self, disabled: bool) -> impl Iterator<Item = WidgetHolder> + use<> {
		[(FlipAxis::X, "Horizontal"), (FlipAxis::Y, "Vertical")].into_iter().map(move |(flip_axis, name)| {
			IconButton::new("Flip".to_string() + name, 24)
				.tooltip("Flip ".to_string() + name)
				.on_update(move |_| DocumentMessage::FlipSelectedLayers { flip_axis }.into())
				.disabled(disabled)
				.widget_holder()
		})
	}

	fn turn_widgets(&self, disabled: bool) -> impl Iterator<Item = WidgetHolder> + use<> {
		[(-90., "TurnNegative90", "Turn -90°"), (90., "TurnPositive90", "Turn 90°")]
			.into_iter()
			.map(move |(degrees, icon, name)| {
				IconButton::new(icon, 24)
					.tooltip(name)
					.on_update(move |_| DocumentMessage::RotateSelectedLayers { degrees }.into())
					.disabled(disabled)
					.widget_holder()
			})
	}

	fn boolean_widgets(&self, selected_count: usize) -> impl Iterator<Item = WidgetHolder> + use<> {
		let list = <BooleanOperation as graphene_std::choice_type::ChoiceTypeStatic>::list();
		list.iter().flat_map(|i| i.iter()).map(move |(operation, info)| {
			let mut tooltip = info.label.to_string();
			if let Some(doc) = info.docstring.as_deref() {
				tooltip.push_str("\n\n");
				tooltip.push_str(doc);
			}
			IconButton::new(info.icon.as_deref().unwrap(), 24)
				.tooltip(tooltip)
				.disabled(selected_count == 0)
				.on_update(move |_| {
					let group_folder_type = GroupFolderType::BooleanOperation(*operation);
					DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
				})
				.widget_holder()
		})
	}

	pub fn update_tool_options(&mut self, option_update: &SelectOptionsUpdate, responses: &mut VecDeque<Message>) {
		match option_update {
			SelectOptionsUpdate::NestedSelectionBehavior(nested_selection_behavior) => {
				self.tool_data.nested_selection_behavior = *nested_selection_behavior;
				responses.add(ToolMessage::UpdateHints);
			}
			SelectOptionsUpdate::PivotGizmoType(gizmo_type) => {
				if !self.tool_data.pivot_gizmo.state.disabled {
					self.tool_data.pivot_gizmo.state.gizmo_type = *gizmo_type;
					responses.add(ToolMessage::UpdateHints);
					let pivot_gizmo = self.tool_data.pivot_gizmo();
					responses.add(TransformLayerMessage::SetPivotGizmo { pivot_gizmo });
					responses.add(NodeGraphMessage::RunDocumentGraph);
					self.tool_data.pivot_changed = true;
				}
			}
			SelectOptionsUpdate::TogglePivotGizmoType(state) => {
				self.tool_data.pivot_gizmo.state.disabled = !state;
				responses.add(ToolMessage::UpdateHints);
				responses.add(NodeGraphMessage::RunDocumentGraph);
				self.tool_data.pivot_changed = true;
			}

			SelectOptionsUpdate::TogglePivotPinned => {
				self.tool_data.pivot_gizmo.pivot.pinned = !self.tool_data.pivot_gizmo.pivot.pinned;
				responses.add(ToolMessage::UpdateHints);
				responses.add(NodeGraphMessage::RunDocumentGraph);
				self.tool_data.pivot_changed = true;
			}
		}
	}
}

impl LayoutHolder for SelectTool {
	fn layout(&self) -> Layout {
		let mut widgets = Vec::new();

		// Select mode (Deep/Shallow)
		widgets.push(self.deep_selection_widget());

		// Pivot gizmo type (checkbox + dropdown for pivot/origin)
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.extend(pivot_gizmo_type_widget(self.tool_data.pivot_gizmo.state, PivotToolSource::Select));

		if self.tool_data.pivot_gizmo.state.is_pivot_type() {
			// Nine-position reference point widget
			widgets.push(Separator::new(SeparatorType::Related).widget_holder());
			widgets.push(pivot_reference_point_widget(
				self.tool_data.selected_layers_count == 0 || !self.tool_data.pivot_gizmo.state.is_pivot(),
				self.tool_data.pivot_gizmo.pivot.to_pivot_position(),
				PivotToolSource::Select,
			));

			// Pivot pin button
			widgets.push(Separator::new(SeparatorType::Related).widget_holder());

			let pin_active = self.tool_data.pivot_gizmo.pin_active();
			let pin_enabled = self.tool_data.pivot_gizmo.pivot.old_pivot_position == ReferencePoint::None && !self.tool_data.pivot_gizmo.state.disabled;

			if pin_active || pin_enabled {
				widgets.push(pin_pivot_widget(pin_active, pin_enabled, PivotToolSource::Select));
			}
		}

		// Align
		let disabled = self.tool_data.selected_layers_count < 2;
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.extend(self.alignment_widgets(disabled));
		// widgets.push(
		// 	PopoverButton::new()
		// 		.popover_layout(vec![
		// 			LayoutGroup::Row {
		// 				widgets: vec![TextLabel::new("Align").bold(true).widget_holder()],
		// 			},
		// 			LayoutGroup::Row {
		// 				widgets: vec![TextLabel::new("Coming soon").widget_holder()],
		// 			},
		// 		])
		// 		.disabled(disabled)
		// 		.widget_holder(),
		// );

		// Flip
		let disabled = self.tool_data.selected_layers_count == 0;
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.extend(self.flip_widgets(disabled));

		// Turn
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.extend(self.turn_widgets(disabled));

		// Boolean
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.extend(self.boolean_widgets(self.tool_data.selected_layers_count));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}
