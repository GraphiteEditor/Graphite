//! Handler for the pivot overlay visible on the selected layer(s) whilst using the Select tool which controls the center of rotation/scale.

use crate::consts::PIVOT_DIAMETER;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::path_tool::PathOptionsUpdate;
use crate::messages::tool::tool_messages::select_tool::SelectOptionsUpdate;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::{DAffine2, DVec2};
use graphene_std::transform::ReferencePoint;
use graphene_std::vector::misc::ManipulatorPointId;
use std::fmt;

pub fn pin_pivot_widget(active: bool, enabled: bool, source: PivotToolSource) -> WidgetHolder {
	IconButton::new(if active { "PinActive" } else { "PinInactive" }, 24)
		.tooltip(String::from(if active { "Unpin Custom Pivot" } else { "Pin Custom Pivot" }) + "\n\nUnless pinned, the pivot will return to its prior reference point when a new selection is made.")
		.disabled(!enabled)
		.on_update(move |_| match source {
			PivotToolSource::Select => SelectToolMessage::SelectOptions(SelectOptionsUpdate::TogglePivotPinned).into(),
			PivotToolSource::Path => PathToolMessage::UpdateOptions(PathOptionsUpdate::TogglePivotPinned).into(),
		})
		.widget_holder()
}

pub fn pivot_reference_point_widget(disabled: bool, reference_point: ReferencePoint, source: PivotToolSource) -> WidgetHolder {
	ReferencePointInput::new(reference_point)
		.tooltip("Custom Pivot Reference Point\n\nPlaces the pivot at a corner, edge, or center of the selection bounds, unless it is dragged elsewhere.")
		.disabled(disabled)
		.on_update(move |pivot_input: &ReferencePointInput| match source {
			PivotToolSource::Select => SelectToolMessage::SetPivot { position: pivot_input.value }.into(),
			PivotToolSource::Path => PathToolMessage::SetPivot { position: pivot_input.value }.into(),
		})
		.widget_holder()
}

pub fn pivot_gizmo_type_widget(state: PivotGizmoState, source: PivotToolSource) -> Vec<WidgetHolder> {
	let gizmo_type_entries = [PivotGizmoType::Pivot, PivotGizmoType::Average, PivotGizmoType::Active]
		.iter()
		.map(|gizmo_type| {
			MenuListEntry::new(format!("{gizmo_type:?}")).label(gizmo_type.to_string()).on_commit({
				let value = source.clone();
				move |_| match value {
					PivotToolSource::Select => SelectToolMessage::SelectOptions(SelectOptionsUpdate::PivotGizmoType(*gizmo_type)).into(),
					PivotToolSource::Path => PathToolMessage::UpdateOptions(PathOptionsUpdate::PivotGizmoType(*gizmo_type)).into(),
				}
			})
		})
		.collect();

	vec![
		CheckboxInput::new(!state.disabled)
			.tooltip(
				"Pivot Gizmo\n\
				\n\
				Enabled: the chosen gizmo type is shown and used to control rotation and scaling.\n\
				Disabled: rotation and scaling occurs about the center of the selection bounds.",
			)
			.on_update(move |optional_input: &CheckboxInput| match source {
				PivotToolSource::Select => SelectToolMessage::SelectOptions(SelectOptionsUpdate::TogglePivotGizmoType(optional_input.checked)).into(),
				PivotToolSource::Path => PathToolMessage::UpdateOptions(PathOptionsUpdate::TogglePivotGizmoType(optional_input.checked)).into(),
			})
			.widget_holder(),
		Separator::new(SeparatorType::Related).widget_holder(),
		DropdownInput::new(vec![gizmo_type_entries])
			.selected_index(Some(match state.gizmo_type {
				PivotGizmoType::Pivot => 0,
				PivotGizmoType::Average => 1,
				PivotGizmoType::Active => 2,
			}))
			.tooltip(
				"Pivot Gizmo Type\n\
				\n\
				Selects which gizmo type is shown and used as the center of rotation/scaling transformations.\n\
				\n\
				Custom Pivot: rotates and scales relative to the selection bounds, or elsewhere if dragged.\n\
				Origin (Average Point): rotates and scales about the average point of all selected layer origins.\n\
				Origin (Active Object): rotates and scales about the origin of the most recently selected layer.",
			)
			.disabled(state.disabled)
			.widget_holder(),
	]
}

#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum PivotToolSource {
	Path,
	#[default]
	Select,
}

#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PivotGizmo {
	pub pivot: Pivot,
	pub state: PivotGizmoState,
	pub layer: Option<LayerNodeIdentifier>,
	pub point: Option<ManipulatorPointId>,
}

impl PivotGizmo {
	pub fn position(&self, document: &DocumentMessageHandler) -> DVec2 {
		let network = &document.network_interface;
		(!self.state.disabled)
			.then_some({
				match self.state.gizmo_type {
					PivotGizmoType::Average => Some(network.selected_nodes().selected_visible_and_unlocked_layers_mean_average_origin(network)),
					PivotGizmoType::Pivot => self.pivot.pivot,
					PivotGizmoType::Active => self.layer.map(|layer| graph_modification_utils::get_viewport_origin(layer, network)),
				}
			})
			.flatten()
			.unwrap_or_else(|| self.pivot.transform_from_normalized.transform_point2(DVec2::splat(0.5)))
	}

	pub fn recalculate_transform(&mut self, document: &DocumentMessageHandler) -> DAffine2 {
		self.pivot.recalculate_pivot(document);
		self.pivot.transform_from_normalized
	}

	pub fn pin_active(&self) -> bool {
		self.pivot.pinned && self.state.is_pivot_type()
	}

	pub fn pivot_disconnected(&self) -> bool {
		self.pivot.old_pivot_position == ReferencePoint::None
	}
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PivotGizmoType {
	// Pivot
	#[default]
	Pivot,
	// Origin
	Average,
	Active,
	// TODO: Add "Individual"
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PivotGizmoState {
	pub disabled: bool,
	pub gizmo_type: PivotGizmoType,
}

impl PivotGizmoState {
	pub fn is_pivot_type(&self) -> bool {
		self.gizmo_type == PivotGizmoType::Pivot || self.disabled
	}

	pub fn is_pivot(&self) -> bool {
		self.gizmo_type == PivotGizmoType::Pivot && !self.disabled
	}
}

impl fmt::Display for PivotGizmoType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			PivotGizmoType::Pivot => write!(f, "Custom Pivot"),
			PivotGizmoType::Average => write!(f, "Origin (Average Point)"),
			PivotGizmoType::Active => write!(f, "Origin (Active Object)"),
			// TODO: Add "Origin (Individual)"
		}
	}
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Pivot {
	/// Pivot between (0,0) and (1,1)
	normalized_pivot: DVec2,
	/// Transform to get from normalized pivot to viewspace
	pub transform_from_normalized: DAffine2,
	/// The viewspace pivot position
	pub pivot: Option<DVec2>,
	/// The old pivot position in the GUI, used to reduce refreshes of the document bar
	pub old_pivot_position: ReferencePoint,
	/// The last ReferencePoint which wasn't none
	pub last_non_none_reference_point: ReferencePoint,
	/// Used to enable and disable the pivot
	pub pinned: bool,
	/// Had selected_visible_and_unlocked_layers
	pub empty: bool,
}

impl Default for Pivot {
	fn default() -> Self {
		Self {
			normalized_pivot: DVec2::splat(0.5),
			transform_from_normalized: Default::default(),
			pivot: Default::default(),
			old_pivot_position: ReferencePoint::Center,
			last_non_none_reference_point: ReferencePoint::Center,
			pinned: false,
			empty: true,
		}
	}
}

impl Pivot {
	/// Recomputes the pivot position and transform.
	pub fn recalculate_pivot(&mut self, document: &DocumentMessageHandler) {
		let selected = document.network_interface.selected_nodes();
		self.empty = !selected.has_selected_nodes();
		if !selected.has_selected_nodes() {
			return;
		}

		let transform = selected
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.find(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
			.map(|layer| document.metadata().transform_to_viewport_with_first_transform_node_if_group(layer, &document.network_interface))
			.unwrap_or_default();

		let bounds = document
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.filter_map(|layer| {
				document
					.metadata()
					.bounding_box_with_transform(layer, transform.inverse() * document.metadata().transform_to_viewport(layer))
			})
			.reduce(graphene_std::renderer::Quad::combine_bounds);

		let [min, max] = bounds.unwrap_or([DVec2::ZERO, DVec2::ONE]);
		self.transform_from_normalized = transform * DAffine2::from_translation(min) * DAffine2::from_scale(max - min);

		if self.old_pivot_position != ReferencePoint::None {
			self.pivot = Some(self.transform_from_normalized.transform_point2(self.normalized_pivot));
		}
	}

	pub fn recalculate_pivot_for_layer(&mut self, document: &DocumentMessageHandler, bounds: Option<[DVec2; 2]>) {
		let selected = document.network_interface.selected_nodes();
		if !selected.has_selected_nodes() {
			self.normalized_pivot = DVec2::splat(0.5);
			self.pivot = None;
			return;
		};

		let [min, max] = bounds.unwrap_or([DVec2::ZERO, DVec2::ONE]);
		self.transform_from_normalized = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		self.pivot = Some(self.transform_from_normalized.transform_point2(self.normalized_pivot));
	}

	/// Answers if the pivot widget has changed (so we should refresh the tool bar at the top of the canvas).
	pub fn should_refresh_pivot_position(&mut self) -> bool {
		let new = self.to_pivot_position();
		let should_refresh = new != self.old_pivot_position;
		self.old_pivot_position = new;
		should_refresh
	}

	pub fn to_pivot_position(&self) -> ReferencePoint {
		self.normalized_pivot.into()
	}

	/// Sets the viewport position of the pivot.
	pub fn set_viewport_position(&mut self, position: DVec2) {
		if self.transform_from_normalized.matrix2.determinant().abs() <= f64::EPSILON {
			return;
		};

		self.normalized_pivot = self.transform_from_normalized.inverse().transform_point2(position);
		self.pivot = Some(position);
	}

	/// Set the pivot using a normalized position.
	pub fn set_normalized_position(&mut self, position: DVec2) {
		self.normalized_pivot = position;
		self.pivot = Some(self.transform_from_normalized.transform_point2(position));
	}

	/// Answers if the pointer is currently positioned over the pivot.
	pub fn is_over(&self, mouse: DVec2) -> bool {
		self.pivot.filter(|&pivot| mouse.distance_squared(pivot) < (PIVOT_DIAMETER / 2.).powi(2)).is_some()
	}
}
