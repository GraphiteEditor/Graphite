use crate::messages::color_picker::color_picker_message::{HsvChannel, RgbChannel};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::layout::utility_types::widgets::input_widgets::{ColorPresetsInputUpdate, SpectrumInputUpdate, SpectrumMarker, VisualColorPickersInputUpdate};
use crate::messages::prelude::*;
use color::{AlphaColor, Srgb};
use graphene_std::Color;
use graphene_std::vector::style::{FillChoice, GradientStops};

/// Bounds for a midpoint position (relative to the interval between two adjacent gradient stops).
const MIN_MIDPOINT: f64 = 0.01;
const MAX_MIDPOINT: f64 = 0.99;

#[derive(Debug, Clone, PartialEq, ExtractField)]
pub struct ColorPickerMessageHandler {
	// HSV is the source of truth so the hue is preserved when the user desaturates the color (or drives the value to black) and back.
	hue: f64,
	saturation: f64,
	value: f64,
	alpha: f64,
	is_none: bool,

	// Snapshot of the color when the picker opened, used by the new/old comparison swatch and the swap button.
	old_hue: f64,
	old_saturation: f64,
	old_value: f64,
	old_alpha: f64,
	old_is_none: bool,

	// When set, the picker is editing a gradient: the visual pickers and inputs target the active stop's color.
	gradient: Option<GradientStops>,
	active_marker_index: Option<u32>,
	active_marker_is_midpoint: bool,

	allow_none: bool,
	disabled: bool,
}

impl Default for ColorPickerMessageHandler {
	fn default() -> Self {
		Self {
			hue: 0.,
			saturation: 0.,
			value: 0.,
			alpha: 1.,
			is_none: true,
			old_hue: 0.,
			old_saturation: 0.,
			old_value: 0.,
			old_alpha: 1.,
			old_is_none: true,
			gradient: None,
			active_marker_index: None,
			active_marker_is_midpoint: false,
			allow_none: true,
			disabled: false,
		}
	}
}

#[message_handler_data]
impl MessageHandler<ColorPickerMessage, ()> for ColorPickerMessageHandler {
	fn process_message(&mut self, message: ColorPickerMessage, responses: &mut VecDeque<Message>, _context: ()) {
		match message {
			ColorPickerMessage::Open { initial_value, allow_none, disabled } => {
				self.allow_none = allow_none;
				self.disabled = disabled;

				// Each `<ColorPicker>` Svelte instance maintains its own local layout state, but the Rust `LayoutMessageHandler` keeps a single shared layout per target. When a new picker instance opens after a previous one closed, the new instance's layout starts empty and a diff from the previously-shared state would not apply. Destroying the stored layouts here forces the next `SendLayout` to send the full layout instead of a diff.
				responses.add(LayoutMessage::DestroyLayout {
					layout_target: LayoutTarget::ColorPickerPickersAndGradient,
				});
				responses.add(LayoutMessage::DestroyLayout {
					layout_target: LayoutTarget::ColorPickerDetails,
				});

				match initial_value {
					FillChoice::None => {
						self.set_new_hsva(0., 0., 0., 1., true);
						self.gradient = None;
						self.active_marker_index = None;
						self.active_marker_is_midpoint = false;
					}
					FillChoice::Solid(color) => {
						self.gradient = None;
						self.active_marker_index = None;
						self.active_marker_is_midpoint = false;
						self.adopt_color(color);
					}
					FillChoice::Gradient(stops) => {
						self.active_marker_index = Some(0);
						self.active_marker_is_midpoint = false;
						let first_color = stops.color.first().copied().unwrap_or(Color::BLACK);
						self.gradient = Some(stops);
						self.adopt_color(first_color);
					}
				}

				self.snapshot_old();
				self.send_layouts(responses);
			}
			ColorPickerMessage::Close => {
				self.gradient = None;
				self.active_marker_index = None;
				self.active_marker_is_midpoint = false;
			}
			ColorPickerMessage::VisualUpdate { update } => {
				self.hue = update.hue;
				self.saturation = update.saturation;
				self.value = update.value;
				self.alpha = update.alpha;
				self.is_none = false;
				self.emit_color(responses);
				self.send_layouts(responses);
			}
			ColorPickerMessage::SetChannelRgb { channel, value } => {
				let Some(strength) = value else { return };
				let Some(current) = self.current_color() else { return };
				let updated = match channel {
					RgbChannel::Red => Color::from_rgbaf32_unchecked((strength / 255.) as f32, current.g(), current.b(), current.a()),
					RgbChannel::Green => Color::from_rgbaf32_unchecked(current.r(), (strength / 255.) as f32, current.b(), current.a()),
					RgbChannel::Blue => Color::from_rgbaf32_unchecked(current.r(), current.g(), (strength / 255.) as f32, current.a()),
				};
				self.adopt_color(updated);
				self.emit_color(responses);
				self.send_layouts(responses);
			}
			ColorPickerMessage::SetChannelHsv { channel, value } => {
				let Some(strength) = value else { return };
				match channel {
					HsvChannel::Hue => self.hue = strength / 360.,
					HsvChannel::Saturation => self.saturation = strength / 100.,
					HsvChannel::Value => self.value = strength / 100.,
				}
				self.is_none = false;
				self.emit_color(responses);
				self.send_layouts(responses);
			}
			ColorPickerMessage::SetAlphaPercent { value } => {
				let Some(strength) = value else { return };
				self.alpha = strength / 100.;
				self.is_none = false;
				self.emit_color(responses);
				self.send_layouts(responses);
			}
			ColorPickerMessage::SetHexCode { code } => {
				let Some(color) = parse_css_color(&code) else {
					// Parse failed: re-send the layouts so the TextInput's displayed value reverts from the user's bad input
					// back to the current color's hex string. The TextInput dispatch arm has already mutated the stored
					// widget's `value` to the bad input, so the diff between (stored = bad) and (new = correct) sends an update.
					self.send_layouts(responses);
					return;
				};
				responses.add(FrontendMessage::ColorPickerStartHistoryTransaction);
				self.adopt_color(color);
				self.emit_color(responses);
				self.send_layouts(responses);
			}
			ColorPickerMessage::PickPreset { preset } => {
				responses.add(FrontendMessage::ColorPickerStartHistoryTransaction);
				match preset {
					FillChoice::None => {
						self.set_new_hsva(0., 0., 0., 1., true);
						responses.add(FrontendMessage::ColorPickerColorChanged { value: FillChoice::None });
					}
					FillChoice::Solid(color) => {
						self.adopt_color(color);
						self.emit_color(responses);
					}
					FillChoice::Gradient(_) => {
						// The presets row only emits solid colors or "None"; the gradient case is unreachable, so safely ignore.
					}
				}
				self.send_layouts(responses);
			}
			ColorPickerMessage::EyedropperColorCode { code } => {
				let Some(color) = parse_css_color(&code) else { return };
				responses.add(FrontendMessage::ColorPickerStartHistoryTransaction);
				self.adopt_color(color);
				self.emit_color(responses);
				self.send_layouts(responses);
			}
			ColorPickerMessage::SwapNewWithOld => {
				let temp = (self.hue, self.saturation, self.value, self.alpha, self.is_none);
				self.set_new_hsva(self.old_hue, self.old_saturation, self.old_value, self.old_alpha, self.old_is_none);
				self.set_old_hsva(temp.0, temp.1, temp.2, temp.3, temp.4);

				if self.is_none {
					responses.add(FrontendMessage::ColorPickerColorChanged { value: FillChoice::None });
				} else {
					self.emit_color(responses);
				}
				self.send_layouts(responses);
			}
			ColorPickerMessage::GradientUpdate { update } => self.apply_gradient_update(update, responses),
			ColorPickerMessage::StartTransaction => {
				responses.add(FrontendMessage::ColorPickerStartHistoryTransaction);
			}
			ColorPickerMessage::CommitTransaction => {
				responses.add(FrontendMessage::ColorPickerCommitHistoryTransaction);
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(ColorPickerMessageDiscriminant;)
	}
}

impl ColorPickerMessageHandler {
	fn current_color(&self) -> Option<Color> {
		if self.is_none {
			None
		} else {
			Some(Color::from_hsva(self.hue as f32, self.saturation as f32, self.value as f32, self.alpha as f32))
		}
	}

	fn old_color(&self) -> Option<Color> {
		if self.old_is_none {
			None
		} else {
			Some(Color::from_hsva(self.old_hue as f32, self.old_saturation as f32, self.old_value as f32, self.old_alpha as f32))
		}
	}

	fn set_new_hsva(&mut self, h: f64, s: f64, v: f64, a: f64, is_none: bool) {
		self.hue = h;
		self.saturation = s;
		self.value = v;
		self.alpha = a;
		self.is_none = is_none;
	}

	fn set_old_hsva(&mut self, h: f64, s: f64, v: f64, a: f64, is_none: bool) {
		self.old_hue = h;
		self.old_saturation = s;
		self.old_value = v;
		self.old_alpha = a;
		self.old_is_none = is_none;
	}

	fn snapshot_old(&mut self) {
		self.old_hue = self.hue;
		self.old_saturation = self.saturation;
		self.old_value = self.value;
		self.old_alpha = self.alpha;
		self.old_is_none = self.is_none;
	}

	/// Set HSV state from a Color, preserving hue and saturation in degenerate cases.
	fn adopt_color(&mut self, color: Color) {
		let [target_h, target_s, target_v] = rgb_to_hsv(color.r() as f64, color.g() as f64, color.b() as f64);

		// Preserve hue: avoid jumping from 360° (top) to 0° (bottom) and don't reset hue when the color is desaturated or fully dark.
		if !(target_h == 0. && self.hue == 1.) && target_s > 0. && target_v > 0. {
			self.hue = target_h;
		}
		// Preserve saturation when value is black (saturation is meaningless on the bottom edge of the saturation-value box).
		if target_v != 0. {
			self.saturation = target_s;
		}
		self.value = target_v;
		self.alpha = color.a() as f64;
		self.is_none = false;
	}

	/// Compute the FillChoice and forward it via `FrontendMessage::ColorPickerColorChanged`. In gradient mode, the active stop's color is updated in place.
	fn emit_color(&mut self, responses: &mut VecDeque<Message>) {
		let Some(color) = self.current_color() else { return };

		if let Some(gradient) = &mut self.gradient
			&& let Some(active_index) = self.active_marker_index
			&& let Some(stop_color) = gradient.color.get_mut(active_index as usize)
		{
			*stop_color = color;
			let stops = gradient.clone();
			responses.add(FrontendMessage::ColorPickerColorChanged { value: FillChoice::Gradient(stops) });
		} else {
			responses.add(FrontendMessage::ColorPickerColorChanged { value: FillChoice::Solid(color) });
		}
	}

	fn send_layouts(&self, responses: &mut VecDeque<Message>) {
		responses.add(LayoutMessage::SendLayout {
			layout: self.pickers_and_gradient_layout(),
			layout_target: LayoutTarget::ColorPickerPickersAndGradient,
		});
		responses.add(LayoutMessage::SendLayout {
			layout: self.details_layout(),
			layout_target: LayoutTarget::ColorPickerDetails,
		});
	}

	/// Apply an incoming `SpectrumInput` intent to the gradient state and broadcast the result
	fn apply_gradient_update(&mut self, update: SpectrumInputUpdate, responses: &mut VecDeque<Message>) {
		// Active marker selection is the one update that doesn't mutate the gradient
		if let SpectrumInputUpdate::ActiveMarker {
			active_marker_index,
			active_marker_is_midpoint,
		} = update
		{
			self.active_marker_index = active_marker_index;
			self.active_marker_is_midpoint = active_marker_is_midpoint;
			if let Some(index) = active_marker_index
				&& let Some(gradient) = &self.gradient
				&& let Some(color) = gradient.color.get(index as usize).copied()
			{
				self.adopt_color(color);
				self.snapshot_old();
			}
			self.send_layouts(responses);
			return;
		}

		let Some(mut gradient) = self.gradient.clone() else { return };

		match update {
			SpectrumInputUpdate::MoveMarker { index, position } => {
				let new_index = gradient.move_stop(index as usize, position);
				if Some(index) == self.active_marker_index {
					self.active_marker_index = Some(new_index as u32);
				}
			}
			SpectrumInputUpdate::MoveMidpoint { index, position } => {
				if let Some(midpoint) = gradient.midpoint.get_mut(index as usize) {
					*midpoint = position.clamp(MIN_MIDPOINT, MAX_MIDPOINT);
				} else {
					return;
				}
			}
			SpectrumInputUpdate::InsertMarker { position } => {
				let new_index = gradient.insert_stop(position);
				self.active_marker_index = Some(new_index as u32);
				self.active_marker_is_midpoint = false;
				if let Some(color) = gradient.color.get(new_index).copied() {
					self.adopt_color(color);
					self.snapshot_old();
				}
			}
			SpectrumInputUpdate::DeleteMarker { index } => {
				// Enforce minimum stop count. The gradient editor needs at least 2 stops to remain meaningful.
				if gradient.position.len() <= 2 {
					return;
				}
				gradient.remove(index as usize);
				let new_active = (index as usize).min(gradient.position.len() - 1);
				self.active_marker_index = Some(new_active as u32);
				self.active_marker_is_midpoint = false;
				if let Some(color) = gradient.color.get(new_active).copied() {
					self.adopt_color(color);
					self.snapshot_old();
				}
			}
			SpectrumInputUpdate::ResetMidpoint { index } => {
				gradient.reset_midpoint(index as usize);
			}
			SpectrumInputUpdate::ActiveMarker { .. } => unreachable!("handled above"),
		}

		self.gradient = Some(gradient.clone());
		responses.add(FrontendMessage::ColorPickerColorChanged {
			value: FillChoice::Gradient(gradient),
		});
		self.send_layouts(responses);
	}

	fn pickers_and_gradient_layout(&self) -> Layout {
		let mut groups = Vec::new();

		// Visual H/S/V/A pickers
		groups.push(LayoutGroup::row(vec![
			VisualColorPickersInput::default()
				.hue(self.hue)
				.saturation(self.saturation)
				.value(self.value)
				.alpha(self.alpha)
				.is_none(self.is_none)
				.disabled(self.disabled)
				.on_update(|update: &VisualColorPickersInputUpdate| ColorPickerMessage::VisualUpdate { update: update.clone() }.into())
				.on_commit(|_| ColorPickerMessage::CommitTransaction.into())
				.widget_instance(),
		]));

		// Gradient editor (only present when the picker is in gradient mode)
		if let Some(gradient) = &self.gradient {
			// For gradient editing, the markers' handle colors mirror their gradient stop colors
			let markers = gradient.iter().map(|stop| SpectrumMarker::new(stop.position, stop.midpoint, stop.color)).collect();
			let mut row_widgets = vec![
				SpectrumInput::new(gradient.clone())
					.markers(markers)
					.active_marker_index(self.active_marker_index)
					.active_marker_is_midpoint(self.active_marker_is_midpoint)
					.show_midpoints(true)
					.allow_insert(!self.disabled)
					.allow_delete(!self.disabled)
					.allow_swap(true)
					.disabled(self.disabled)
					.on_update(|update: &SpectrumInputUpdate| ColorPickerMessage::GradientUpdate { update: update.clone() }.into())
					.widget_instance(),
			];

			if let Some(active) = self.active_marker_index {
				let active_index = active as usize;
				let position_value = if self.active_marker_is_midpoint {
					gradient.midpoint.get(active_index).copied().unwrap_or(0.)
				} else {
					gradient.position.get(active_index).copied().unwrap_or(0.)
				};
				let is_midpoint = self.active_marker_is_midpoint;
				let captured_index = active;
				row_widgets.push(
					NumberInput::new(Some(position_value * 100.))
						.disabled(self.disabled)
						.display_decimal_places(0)
						.min(if is_midpoint { 1. } else { 0. })
						.max(if is_midpoint { 99. } else { 100. })
						.unit("%")
						.on_update(move |widget: &NumberInput| {
							let Some(new_value) = widget.value else {
								return Message::NoOp;
							};
							let update = if is_midpoint {
								SpectrumInputUpdate::MoveMidpoint {
									index: captured_index,
									position: new_value / 100.,
								}
							} else {
								SpectrumInputUpdate::MoveMarker {
									index: captured_index,
									position: new_value / 100.,
								}
							};
							ColorPickerMessage::GradientUpdate { update }.into()
						})
						.widget_instance(),
				);
			}

			groups.push(LayoutGroup::row(row_widgets));
		}

		Layout(groups)
	}

	fn details_layout(&self) -> Layout {
		let new_color = self.current_color();
		let old_color = self.old_color();

		let hex_value = new_color.map(|c| color_to_hex_optional_alpha(&c)).unwrap_or_else(|| "-".to_string());
		let rgb_255 = new_color.map(|c| (c.r() as f64 * 255., c.g() as f64 * 255., c.b() as f64 * 255.));

		let differs = new_color != old_color;
		let outline_amount = contrasting_outline_factor(new_color).max(contrasting_outline_factor(old_color));

		let mut groups = Vec::new();

		// New/old comparison swatch with swap button
		groups.push(LayoutGroup::row(vec![
			ColorComparisonInput::new(new_color, old_color)
				.is_none(self.is_none)
				.old_is_none(self.old_is_none)
				.disabled(self.disabled)
				.differs(differs)
				.outline_amount(outline_amount)
				.on_update(|_: &()| ColorPickerMessage::SwapNewWithOld.into())
				.widget_instance(),
		]));

		// Hex
		groups.push(LayoutGroup::row(vec![
			TextLabel::new("Hex").tooltip_label("Hex Color Code").tooltip_description(HEX_DESCRIPTION).widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			TextInput::new(hex_value)
				.centered(true)
				.disabled(self.disabled)
				.tooltip_label("Hex Color Code")
				.tooltip_description(HEX_DESCRIPTION)
				.on_update(|widget: &TextInput| ColorPickerMessage::SetHexCode { code: widget.value.clone() }.into())
				.widget_instance(),
		]));

		// RGB
		groups.push(LayoutGroup::row(vec![
			TextLabel::new("RGB").tooltip_label("Red/Green/Blue").tooltip_description("Integers 0–255.").widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			rgb_input(RgbChannel::Red, rgb_255.map(|(r, _, _)| r), "Red Channel", self.disabled),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			rgb_input(RgbChannel::Green, rgb_255.map(|(_, g, _)| g), "Green Channel", self.disabled),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			rgb_input(RgbChannel::Blue, rgb_255.map(|(_, _, b)| b), "Blue Channel", self.disabled),
		]));

		// HSV
		groups.push(LayoutGroup::row(vec![
			TextLabel::new("HSV")
				.tooltip_label("Hue/Saturation/Value")
				.tooltip_description("Also known as Hue/Saturation/Brightness (HSB). Not to be confused with Hue/Saturation/Lightness (HSL), a different color model.")
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			hsv_input(
				HsvChannel::Hue,
				if self.is_none { None } else { Some(self.hue * 360.) },
				360.,
				"°",
				"Hue Component",
				HUE_DESCRIPTION,
				self.disabled,
			),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			hsv_input(
				HsvChannel::Saturation,
				if self.is_none { None } else { Some(self.saturation * 100.) },
				100.,
				"%",
				"Saturation Component",
				SATURATION_DESCRIPTION,
				self.disabled,
			),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			hsv_input(
				HsvChannel::Value,
				if self.is_none { None } else { Some(self.value * 100.) },
				100.,
				"%",
				"Value Component",
				VALUE_DESCRIPTION,
				self.disabled,
			),
		]));

		// Alpha
		groups.push(LayoutGroup::row(vec![
			TextLabel::new("Alpha").tooltip_label("Alpha").tooltip_description(ALPHA_DESCRIPTION).widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			NumberInput::new(if self.is_none { None } else { Some(self.alpha * 100.) })
				.disabled(self.disabled)
				.min(0.)
				.max(100.)
				.mode_range()
				.unit("%")
				.display_decimal_places(1)
				.tooltip_label("Alpha")
				.tooltip_description(ALPHA_DESCRIPTION)
				.on_update(|widget: &NumberInput| ColorPickerMessage::SetAlphaPercent { value: widget.value }.into())
				.on_commit(|_| ColorPickerMessage::StartTransaction.into())
				.widget_instance(),
		]));

		// Color presets (None / Black / White / pure colors / eyedropper)
		groups.push(LayoutGroup::row(vec![
			ColorPresetsInput::default()
				.disabled(self.disabled)
				.show_none_option(self.allow_none && self.gradient.is_none())
				.on_update(|update: &ColorPresetsInputUpdate| match update {
					ColorPresetsInputUpdate::Preset(fill_choice) => ColorPickerMessage::PickPreset { preset: fill_choice.clone() }.into(),
					ColorPresetsInputUpdate::EyedropperColorCode(code) => ColorPickerMessage::EyedropperColorCode { code: code.clone() }.into(),
				})
				.widget_instance(),
		]));

		Layout(groups)
	}
}

fn rgb_input(channel: RgbChannel, value: Option<f64>, tooltip_label: &'static str, disabled: bool) -> WidgetInstance {
	NumberInput::new(value)
		.disabled(disabled)
		.min(0.)
		.max(255.)
		.min_width(1)
		.display_decimal_places(0)
		.tooltip_label(tooltip_label)
		.tooltip_description("Integers 0–255.")
		.on_update(move |widget: &NumberInput| ColorPickerMessage::SetChannelRgb { channel, value: widget.value }.into())
		.on_commit(|_| ColorPickerMessage::StartTransaction.into())
		.widget_instance()
}

fn hsv_input(channel: HsvChannel, value: Option<f64>, max: f64, unit: &'static str, tooltip_label: &'static str, tooltip_description: &'static str, disabled: bool) -> WidgetInstance {
	NumberInput::new(value)
		.disabled(disabled)
		.min(0.)
		.max(max)
		.min_width(1)
		.unit(unit)
		.display_decimal_places(1)
		.tooltip_label(tooltip_label)
		.tooltip_description(tooltip_description)
		.on_update(move |widget: &NumberInput| ColorPickerMessage::SetChannelHsv { channel, value: widget.value }.into())
		.on_commit(|_| ColorPickerMessage::StartTransaction.into())
		.widget_instance()
}

const HEX_DESCRIPTION: &str = "Color code in hexadecimal format. 6 digits if opaque, 8 with alpha. Accepts input of CSS color values including named colors.";
const HUE_DESCRIPTION: &str = "The shade along the spectrum of the rainbow.";
const SATURATION_DESCRIPTION: &str = "The vividness from grayscale to full color.";
const VALUE_DESCRIPTION: &str = "The brightness from black to full color.";
const ALPHA_DESCRIPTION: &str = "The level of translucency, from transparent (0%) to opaque (100%).";

/// Convert an `rgb(0..1)` triple to `hsv(0..1)`. Mirrors the legacy frontend `colorToHSV`.
fn rgb_to_hsv(red: f64, green: f64, blue: f64) -> [f64; 3] {
	let max = red.max(green).max(blue);
	let min = red.min(green).min(blue);
	let delta = max - min;

	let mut hue = if delta == 0. {
		0.
	} else if max == red {
		((green - blue) / delta).rem_euclid(6.)
	} else if max == green {
		(blue - red) / delta + 2.
	} else {
		(red - green) / delta + 4.
	};
	hue = (hue * 60. + 360.).rem_euclid(360.) / 360.;

	let saturation = if max == 0. { 0. } else { delta / max };
	let value = max;

	[hue, saturation, value]
}

/// The popover's background color (the `--color-2-mildblack` design token, `#222`). Used by the comparison swatch's
/// outline computation to brighten the inset border for colors close to this background.
const POPOVER_BACKGROUND: Color = Color::from_rgbaf32_unchecked(0x22 as f32 / 255., 0x22 as f32 / 255., 0x22 as f32 / 255., 1.);
/// The luminance window (in linear-light) within which a color is considered close enough to the popover background
/// to warrant an outline. Mirrors the `proximityRange` argument the legacy frontend passed to `contrastingOutlineFactor`.
const OUTLINE_PROXIMITY_RANGE: f64 = 0.01;

/// Returns a 0..1 outline strength for the comparison swatch, growing toward 1 as the color's luminance and saturation
/// both approach the popover background's luminance, when a color would otherwise visually blend into the popover.
fn contrasting_outline_factor(color: Option<Color>) -> f64 {
	let Some(color) = color else { return 0. };

	// WCAG-style relative luminance, with alpha composited over white in gamma space
	let luminance = |color: Color| {
		// TODO: Remove the `.to_linear_srgb()` once we move to correctly treating `Color` as linear.
		Color::WHITE
			.alpha_blend(Color::from_unassociated_alpha(color.r(), color.g(), color.b(), color.a()))
			.to_linear_srgb()
			.luminance_srgb() as f64
	};

	let distance = (luminance(POPOVER_BACKGROUND) - luminance(color)).abs().max(0.);
	let proximity = 1. - (distance / OUTLINE_PROXIMITY_RANGE).min(1.);
	let [_, saturation, _] = rgb_to_hsv(color.r() as f64, color.g() as f64, color.b() as f64);
	proximity * (1. - saturation)
}

/// Format a Color as a `#`-prefixed hex string, including the alpha component only if it's not fully opaque.
fn color_to_hex_optional_alpha(color: &Color) -> String {
	format!(
		"#{}",
		if color.a() >= 1. {
			color.to_rgb_hex_srgb_from_gamma()
		} else {
			color.to_rgba_hex_srgb_from_gamma()
		}
	)
}

/// Parse a CSS color string (named color, hex, `rgb(...)`, etc.) into a `Color` using the `color` crate's CSS Color 4 parser.
/// Tries the input as-is first (catches CSS named colors like `red`, `rgb(...)`, and well-formed hex like `#abcdef`), then falls back to treating the input as bare hex with length-based expansion to a CSS-parseable form:
/// - 1 char `f` → `#fff` (CSS 3-char shorthand)
/// - 2 char `ab` → `#ababab` (repeated to 6 chars)
/// - 4 char `abcd` → `#00abcd` (left-padded with `00`)
/// - 5 char `abcde` → `#0abcde` (left-padded with `0`)
/// - 3, 6, 8 char inputs are passed through with a `#` prefix.
fn parse_css_color(input: &str) -> Option<Color> {
	let trimmed = input.trim();

	let parsed = color::parse_color(trimmed).ok().or_else(|| {
		let bare = trimmed.strip_prefix('#').unwrap_or(trimmed);
		if bare.is_empty() || !bare.chars().all(|c| c.is_ascii_hexdigit()) {
			return None;
		}
		let expanded = match bare.len() {
			1 => bare.repeat(3),
			2 => bare.repeat(3),
			4 => format!("00{bare}"),
			5 => format!("0{bare}"),
			_ => bare.to_string(),
		};
		let candidate = format!("#{expanded}");
		// Avoid retrying the exact same string we just failed to parse.
		(candidate != trimmed).then(|| color::parse_color(&candidate).ok()).flatten()
	})?;

	let srgb: AlphaColor<Srgb> = parsed.to_alpha_color();
	let [red, green, blue, alpha] = srgb.components;
	Color::from_rgbaf32(red, green, blue, alpha)
}
