use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use graphene_std::Color;
use graphene_std::vector::style::FillChoice;

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ToolColorType {
	Primary,
	Secondary,
	Custom,
}

/// Color selector widgets seen in [`LayoutTarget::ToolOptions`] bar.
pub struct ToolColorOptions {
	pub custom_color: Option<Color>,
	pub primary_working_color: Option<Color>,
	pub secondary_working_color: Option<Color>,
	pub color_type: ToolColorType,
}

impl Default for ToolColorOptions {
	fn default() -> Self {
		Self {
			color_type: ToolColorType::Primary,
			custom_color: Some(Color::BLACK),
			primary_working_color: Some(Color::BLACK),
			secondary_working_color: Some(Color::WHITE),
		}
	}
}

impl ToolColorOptions {
	pub fn new_primary() -> Self {
		Self::default()
	}

	pub fn new_secondary() -> Self {
		Self {
			color_type: ToolColorType::Secondary,
			..Default::default()
		}
	}

	pub fn new_none() -> Self {
		Self {
			color_type: ToolColorType::Custom,
			custom_color: None,
			..Default::default()
		}
	}

	pub fn active_color(&self) -> Option<Color> {
		match self.color_type {
			ToolColorType::Custom => self.custom_color,
			ToolColorType::Primary => self.primary_working_color,
			ToolColorType::Secondary => self.secondary_working_color,
		}
	}

	pub fn apply_fill(&self, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
		if let Some(color) = self.active_color() {
			let fill = graphene_std::vector::style::Fill::solid(color.to_gamma_srgb());
			responses.add(GraphOperationMessage::FillSet { layer, fill });
		}
	}

	pub fn apply_stroke(&self, weight: f64, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
		if let Some(color) = self.active_color() {
			let stroke = graphene_std::vector::style::Stroke::new(Some(color.to_gamma_srgb()), weight);
			responses.add(GraphOperationMessage::StrokeSet { layer, stroke });
		}
	}

	pub fn create_widgets(
		&self,
		label_text: impl Into<String>,
		color_allow_none: bool,
		reset_callback: impl Fn(&IconButton) -> Message + 'static + Send + Sync,
		radio_callback: fn(ToolColorType) -> WidgetCallback<()>,
		color_callback: impl Fn(&ColorInput) -> Message + 'static + Send + Sync,
	) -> Vec<WidgetHolder> {
		let mut widgets = vec![TextLabel::new(label_text).widget_holder()];

		if !color_allow_none {
			widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		} else {
			let reset = IconButton::new("CloseX", 12)
				.disabled(self.custom_color.is_none() && self.color_type == ToolColorType::Custom)
				.tooltip("Clear Color")
				.on_update(reset_callback);

			widgets.push(Separator::new(SeparatorType::Related).widget_holder());
			widgets.push(reset.widget_holder());
			widgets.push(Separator::new(SeparatorType::Related).widget_holder());
		};

		let entries = vec![
			("WorkingColorsPrimary", "Primary Working Color", ToolColorType::Primary),
			("WorkingColorsSecondary", "Secondary Working Color", ToolColorType::Secondary),
			("CustomColor", "Custom Color", ToolColorType::Custom),
		]
		.into_iter()
		.map(|(icon, tooltip, color_type)| {
			let mut entry = RadioEntryData::new(format!("{color_type:?}")).tooltip(tooltip).icon(icon);
			entry.on_update = radio_callback(color_type);
			entry
		})
		.collect();
		let radio = RadioInput::new(entries).selected_index(Some(self.color_type.clone() as u32)).widget_holder();
		widgets.push(radio);
		widgets.push(Separator::new(SeparatorType::Related).widget_holder());

		let fill_choice = match self.active_color() {
			Some(color) => FillChoice::Solid(color.to_gamma_srgb()),
			None => FillChoice::None,
		};
		let color_button = ColorInput::new(fill_choice).allow_none(color_allow_none).on_update(color_callback);
		widgets.push(color_button.widget_holder());

		widgets
	}
}
