use graphene_core::Color;
use serde::{Deserialize, Serialize};

use crate::messages::layout::utility_types::layout_widget::WidgetCallback;
use crate::messages::layout::utility_types::widget_prelude::{ColorInput, IconButton, RadioEntryData, RadioInput, TextLabel, WidgetHolder};

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum ToolColorType {
	Primary,
	Secondary,
	Custom,
}

pub struct ToolColorOptions {
	pub custom_color: Option<Color>,
	pub primary_working_color: Option<Color>,
	pub secondary_working_color: Option<Color>,
	pub color_type: ToolColorType,
}

impl ToolColorOptions {
	pub fn active_color(&self) -> Option<Color> {
		match self.color_type {
			ToolColorType::Custom => self.custom_color,
			ToolColorType::Primary => self.primary_working_color,
			ToolColorType::Secondary => self.secondary_working_color,
		}
	}

	pub fn create_widgets(
		&self,
		label_text: impl Into<String>,
		reset_callback: WidgetCallback<IconButton>,
		radio_callback: fn(ToolColorType) -> WidgetCallback<()>,
		color_callback: WidgetCallback<ColorInput>,
	) -> Vec<WidgetHolder> {
		let label = TextLabel::new(label_text).widget_holder();

		let mut reset = IconButton::new("CloseX", 12)
			.disabled(self.custom_color.is_none() && self.color_type == ToolColorType::Custom)
			.tooltip("Clear Color");
		reset.on_update = reset_callback;

		let entries = vec![
			("WorkingColorsPrimary", "Primary Working Color", ToolColorType::Primary),
			("WorkingColorsSecondary", "Secondary Working Color", ToolColorType::Secondary),
			("Edit", "Custom Color", ToolColorType::Custom),
		]
		.into_iter()
		.map(|(icon, tooltip, color_type)| {
			let mut entry = RadioEntryData::new("").tooltip(tooltip).icon(icon);
			entry.on_update = radio_callback(color_type);
			entry
		})
		.collect();
		let radio = RadioInput::new(entries).selected_index(self.color_type.clone() as u32).widget_holder();

		let mut color_input = ColorInput::new(self.active_color());
		color_input.on_update = color_callback;

		vec![
			label,
			WidgetHolder::related_separator(),
			reset.widget_holder(),
			WidgetHolder::related_separator(),
			radio,
			WidgetHolder::related_separator(),
			color_input.widget_holder(),
		]
	}
}
