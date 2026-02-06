use crate::consts::{
	CHAR_WIDTH_ESTIMATE, CHECKBOX_WIDTH, DROPDOWN_PADDING, ICON_BUTTON_24_WIDTH, ICON_BUTTON_32_WIDTH, NUMBER_INPUT_WIDTH, OTHER_WIDGET_WIDTH, POPOVER_BUTTON_WIDTH, SEPARATOR_RELATED_WIDTH,
	SEPARATOR_UNRELATED_WIDTH, TEXT_BUTTON_PADDING, WIDGET_GAP, WIDGET_PADDING,
};
use crate::messages::layout::utility_types::widget_prelude::*;

pub fn apply_overflow(layout: Layout, tool_options_width: f64) -> Layout {
	// If we have max width, no overflow needed
	if tool_options_width >= f64::MAX - 1.0 {
		return layout;
	}

	let available_width = tool_options_width;

	// Only process Row layouts
	let Layout(groups) = layout;
	let mut new_groups = Vec::new();

	for layout_group in groups {
		let LayoutGroup::Row { widgets } = layout_group else {
			new_groups.push(layout_group);
			continue;
		};

		let widget_groups = split_widgets_into_groups(widgets);
		if widget_groups.is_empty() {
			continue;
		}

		let group_widths = calculate_group_widths(&widget_groups);
		let split_index = find_overflow_split_index(&widget_groups, &group_widths, available_width);

		let mut final_widgets = Vec::new();

		// Add visible groups
		for (i, group) in widget_groups.iter().take(split_index).enumerate() {
			if i > 0 {
				final_widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
			}
			final_widgets.extend(group.iter().cloned());
		}

		// Add overflow popover if needed
		if split_index < widget_groups.len() {
			let overflow_groups: Vec<Vec<WidgetInstance>> = widget_groups.into_iter().skip(split_index).collect();
			if !overflow_groups.is_empty() {
				final_widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
				final_widgets.push(create_overflow_popover(overflow_groups).widget_instance());
			}
		}

		new_groups.push(LayoutGroup::Row { widgets: final_widgets });
	}

	Layout(new_groups)
}

fn split_widgets_into_groups(widgets: Vec<WidgetInstance>) -> Vec<Vec<WidgetInstance>> {
	let mut widget_groups: Vec<Vec<WidgetInstance>> = Vec::new();
	let mut current_group: Vec<WidgetInstance> = Vec::new();

	for widget in widgets {
		if let Widget::Separator(sep) = &*widget.widget {
			if sep.style == SeparatorStyle::Unrelated {
				if !current_group.is_empty() {
					widget_groups.push(std::mem::take(&mut current_group));
				}
				continue;
			}
		}
		current_group.push(widget);
	}
	if !current_group.is_empty() {
		widget_groups.push(current_group);
	}
	widget_groups
}

fn calculate_group_widths(widget_groups: &[Vec<WidgetInstance>]) -> Vec<f64> {
	widget_groups
		.iter()
		.enumerate()
		.map(|(i, group)| {
			let width = group.iter().map(estimate_widget_width).sum::<f64>();
			let gaps = if !group.is_empty() { (group.len() - 1) as f64 * WIDGET_GAP } else { 0. };
			width + gaps + if i > 0 { SEPARATOR_UNRELATED_WIDTH } else { 0. }
		})
		.collect()
}

/// Estimate width of a widget for overflow calculation
fn estimate_widget_width(widget: &WidgetInstance) -> f64 {
	match &*widget.widget {
		Widget::IconButton(btn) => {
			if btn.size >= 32 {
				ICON_BUTTON_32_WIDTH
			} else {
				ICON_BUTTON_24_WIDTH
			}
		}
		Widget::DropdownInput(dropdown) => {
			let max_entry_length = dropdown.entries.iter().flatten().map(|entry| entry.label.len()).max().unwrap_or(0);
			max_entry_length as f64 * CHAR_WIDTH_ESTIMATE + DROPDOWN_PADDING
		}
		Widget::CheckboxInput(_) => CHECKBOX_WIDTH,
		Widget::NumberInput(_) => NUMBER_INPUT_WIDTH,
		Widget::Separator(sep) => {
			if sep.style == SeparatorStyle::Unrelated {
				SEPARATOR_UNRELATED_WIDTH
			} else {
				SEPARATOR_RELATED_WIDTH
			}
		}
		Widget::PopoverButton(_) => POPOVER_BUTTON_WIDTH,
		Widget::TextLabel(label) => label.value.len() as f64 * CHAR_WIDTH_ESTIMATE + WIDGET_PADDING,
		Widget::TextButton(button) => button.label.len() as f64 * CHAR_WIDTH_ESTIMATE + TEXT_BUTTON_PADDING,
		Widget::ReferencePointInput(_) => 24.0,
		_ => OTHER_WIDGET_WIDTH,
	}
}

fn is_group_collapsible(group: &[WidgetInstance]) -> bool {
	group.iter().all(|w| match &*w.widget {
		Widget::IconButton(_) | Widget::Separator(_) | Widget::PopoverButton(_) => true,
		_ => false,
	})
}

fn find_overflow_split_index(widget_groups: &[Vec<WidgetInstance>], group_widths: &[f64], available_width: f64) -> usize {
	let mut current_width = 0.0;
	let mut split_index = widget_groups.len();

	// Calculate cumulative widths to find split point
	for (i, group_width) in group_widths.iter().enumerate() {
		let group = &widget_groups[i];

		// If group is NOT collapsible (e.g. contains inputs), it MUST be visible.
		if !is_group_collapsible(group) {
			current_width += group_width;
			continue;
		}

		// Collapsible Group Logic
		let width_with_this = current_width + group_width;

		// Check fit
		if width_with_this <= available_width {
			current_width += group_width;
		} else {
			// Overflow triggers here
			split_index = i;
			break;
		}
	}

	// Backtracking to ensure button fits will ensuring we don't drop Pinned items during backtracking
	while split_index < widget_groups.len() && current_width + POPOVER_BUTTON_WIDTH + SEPARATOR_UNRELATED_WIDTH > available_width && split_index > 0 {
		let candidate_index = split_index - 1;
		if !is_group_collapsible(&widget_groups[candidate_index]) {
			// Stop backtracking if we hit a pinned group.
			break;
		}

		split_index -= 1;
		current_width -= group_widths[split_index];
	}
	split_index
}

fn create_overflow_popover(overflow_groups: Vec<Vec<WidgetInstance>>) -> PopoverButton {
	let mut popover_rows = Vec::new();
	for group in overflow_groups {
		popover_rows.push(LayoutGroup::Row { widgets: group });
	}

	PopoverButton {
		icon: Some("VerticalEllipsis".into()),
		popover_layout: Layout(popover_rows),
		tooltip_label: "More options".into(),
		..Default::default()
	}
}
