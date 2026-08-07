use crate::messages::color_picker::color_picker_message::{HsvChannel, RgbChannel};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::layout::utility_types::widgets::input_widgets::{ColorPresetsInputUpdate, SpectrumInputUpdate, SpectrumMarker, VisualColorPickersInputUpdate};
use crate::messages::prelude::*;
use graphene_std::Color;
use graphene_std::color::SRGBA8;
use graphene_std::core_types::misc::parse_css_color;
use graphene_std::vector::style::{FillChoice, Gradient, GradientHueDirection, GradientInterpolation, GradientRamp, GradientSettings, GradientSpace, GradientSpread, GradientStops};

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
	gradient: Option<Gradient>,
	gradient_spread: GradientSpread,
	gradient_space: GradientSpace,
	gradient_cyclic: bool,
	gradient_hue_direction: GradientHueDirection,
	gradient_interpolation: GradientInterpolation,
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
			gradient_spread: GradientSpread::default(),
			gradient_space: GradientSpace::default(),
			gradient_cyclic: false,
			gradient_hue_direction: GradientHueDirection::default(),
			gradient_interpolation: GradientInterpolation::default(),
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

				match initial_value {
					FillChoice::None => {
						self.set_new_hsva(0., 0., 0., 1., true);
						self.gradient = None;
						self.gradient_spread = GradientSpread::default();
						self.gradient_space = GradientSpace::default();
						self.gradient_cyclic = false;
						self.gradient_hue_direction = GradientHueDirection::default();
						self.gradient_interpolation = GradientInterpolation::default();
						self.active_marker_index = None;
						self.active_marker_is_midpoint = false;
					}
					FillChoice::Solid(color) => {
						self.gradient = None;
						self.gradient_spread = GradientSpread::default();
						self.gradient_space = GradientSpace::default();
						self.gradient_cyclic = false;
						self.gradient_hue_direction = GradientHueDirection::default();
						self.gradient_interpolation = GradientInterpolation::default();
						self.active_marker_index = None;
						self.active_marker_is_midpoint = false;
						self.adopt_color(color);
					}
					FillChoice::Gradient(ramp) => {
						self.active_marker_index = Some(0);
						self.active_marker_is_midpoint = false;
						self.gradient_spread = ramp.gradient_spread;
						self.gradient_space = ramp.gradient_space;
						self.gradient_cyclic = ramp.gradient_cyclic;
						self.gradient_hue_direction = ramp.gradient_hue_direction;
						self.gradient_interpolation = ramp.gradient_interpolation;
						let gradient = Gradient::from(ramp);
						let first_color = gradient.color(0).unwrap_or(Color::BLACK);
						self.gradient = Some(gradient);
						self.adopt_color(first_color);
					}
				}

				self.snapshot_old();
				self.send_layouts(responses);
			}
			ColorPickerMessage::Close => {
				responses.add(DocumentMessage::EndTransaction);
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
				// The RGB inputs are 0..255 sRGB display values; substitute the new channel into the gamma triple and lift back to linear for storage.
				let new_gamma_channel = (strength / 255.) as f32;
				let [cur_r, cur_g, cur_b, cur_a] = current.to_gamma_srgb_channels();
				let updated = match channel {
					RgbChannel::Red => Color::from_gamma_srgb_channels(new_gamma_channel, cur_g, cur_b, cur_a),
					RgbChannel::Green => Color::from_gamma_srgb_channels(cur_r, new_gamma_channel, cur_b, cur_a),
					RgbChannel::Blue => Color::from_gamma_srgb_channels(cur_r, cur_g, new_gamma_channel, cur_a),
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
						responses.add(FrontendMessage::ColorPickerColorChanged { value: FillChoice::<SRGBA8>::None });
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
					responses.add(FrontendMessage::ColorPickerColorChanged { value: FillChoice::<SRGBA8>::None });
				} else {
					self.emit_color(responses);
				}
				self.send_layouts(responses);
			}
			ColorPickerMessage::GradientUpdate { update } => self.apply_gradient_update(update, responses),
			ColorPickerMessage::SetGradientSpread { gradient_spread } => {
				let Some(gradient) = &self.gradient else { return };
				responses.add(FrontendMessage::ColorPickerStartHistoryTransaction);
				self.gradient_spread = gradient_spread;
				responses.add(FrontendMessage::ColorPickerColorChanged {
					value: FillChoice::Gradient(GradientRamp {
						gradient_spread,
						gradient_space: self.gradient_space,
						gradient_cyclic: self.gradient_cyclic,
						gradient_hue_direction: self.gradient_hue_direction,
						gradient_interpolation: self.gradient_interpolation,
						..GradientRamp::from(gradient)
					}),
				});
				self.send_layouts(responses);
			}
			ColorPickerMessage::SetGradientCyclic { gradient_cyclic } => {
				let Some(gradient) = &self.gradient else { return };
				responses.add(FrontendMessage::ColorPickerStartHistoryTransaction);
				self.gradient_cyclic = gradient_cyclic;
				responses.add(FrontendMessage::ColorPickerColorChanged {
					value: FillChoice::Gradient(GradientRamp {
						gradient_spread: self.gradient_spread,
						gradient_space: self.gradient_space,
						gradient_cyclic,
						gradient_hue_direction: self.gradient_hue_direction,
						gradient_interpolation: self.gradient_interpolation,
						..GradientRamp::from(gradient)
					}),
				});
				self.send_layouts(responses);
			}
			ColorPickerMessage::SetGradientSpace { gradient_space } => {
				let Some(gradient) = &self.gradient else { return };
				responses.add(FrontendMessage::ColorPickerStartHistoryTransaction);
				self.gradient_space = gradient_space;
				responses.add(FrontendMessage::ColorPickerColorChanged {
					value: FillChoice::Gradient(GradientRamp {
						gradient_spread: self.gradient_spread,
						gradient_space,
						gradient_cyclic: self.gradient_cyclic,
						gradient_hue_direction: self.gradient_hue_direction,
						gradient_interpolation: self.gradient_interpolation,
						..GradientRamp::from(gradient)
					}),
				});
				self.send_layouts(responses);
			}
			ColorPickerMessage::SetGradientHueDirection { gradient_hue_direction } => {
				let Some(gradient) = &self.gradient else { return };
				responses.add(FrontendMessage::ColorPickerStartHistoryTransaction);
				self.gradient_hue_direction = gradient_hue_direction;
				responses.add(FrontendMessage::ColorPickerColorChanged {
					value: FillChoice::Gradient(GradientRamp {
						gradient_spread: self.gradient_spread,
						gradient_space: self.gradient_space,
						gradient_cyclic: self.gradient_cyclic,
						gradient_hue_direction,
						gradient_interpolation: self.gradient_interpolation,
						..GradientRamp::from(gradient)
					}),
				});
				self.send_layouts(responses);
			}
			ColorPickerMessage::SetGradientInterpolation { gradient_interpolation } => {
				let Some(gradient) = &self.gradient else { return };
				responses.add(FrontendMessage::ColorPickerStartHistoryTransaction);
				self.gradient_interpolation = gradient_interpolation;
				responses.add(FrontendMessage::ColorPickerColorChanged {
					value: FillChoice::Gradient(GradientRamp {
						gradient_spread: self.gradient_spread,
						gradient_space: self.gradient_space,
						gradient_cyclic: self.gradient_cyclic,
						gradient_hue_direction: self.gradient_hue_direction,
						gradient_interpolation,
						..GradientRamp::from(gradient)
					}),
				});
				self.send_layouts(responses);
			}
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
	// The picker's internal HSV state is HSV of sRGB display values
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

	/// The whole-ramp settings the picker is currently editing with, for the sampling entry points that need them all.
	fn gradient_settings(&self) -> GradientSettings {
		GradientSettings {
			spread: self.gradient_spread,
			cyclic: self.gradient_cyclic,
			space: self.gradient_space,
			hue_direction: self.gradient_hue_direction,
			interpolation: self.gradient_interpolation,
		}
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
		let [target_h, target_s, target_v, target_a] = color.to_hsva();
		let (target_h, target_s, target_v, target_a) = (target_h as f64, target_s as f64, target_v as f64, target_a as f64);

		// Preserve hue: avoid jumping from 360° (top) to 0° (bottom) and don't reset hue when the color is desaturated or fully dark.
		if !(target_h == 0. && self.hue == 1.) && target_s > 0. && target_v > 0. {
			self.hue = target_h;
		}
		// Preserve saturation when value is black (saturation is meaningless on the bottom edge of the saturation-value box).
		if target_v != 0. {
			self.saturation = target_s;
		}
		self.value = target_v;
		self.alpha = target_a;
		self.is_none = false;
	}

	/// Compute the FillChoice and forward it via `FrontendMessage::ColorPickerColorChanged`. In gradient mode, the active stop's color is updated in place.
	fn emit_color(&mut self, responses: &mut VecDeque<Message>) {
		let Some(color) = self.current_color() else { return };

		if let Some(gradient) = &mut self.gradient
			&& let Some(active_index) = self.active_marker_index
			&& (active_index as usize) < gradient.len()
		{
			gradient.set_color(active_index as usize, color);
			responses.add(FrontendMessage::ColorPickerColorChanged {
				value: FillChoice::Gradient(GradientRamp {
					gradient_spread: self.gradient_spread,
					gradient_space: self.gradient_space,
					gradient_cyclic: self.gradient_cyclic,
					gradient_hue_direction: self.gradient_hue_direction,
					gradient_interpolation: self.gradient_interpolation,
					..GradientRamp::from(&*gradient)
				}),
			});
		} else {
			responses.add(FrontendMessage::ColorPickerColorChanged {
				value: FillChoice::Solid(SRGBA8::from(color)),
			});
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

	/// Apply an incoming `SpectrumInput` intent to the gradient state and broadcast the result.
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
				&& let Some(color) = gradient.color(index as usize)
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
				let new_index = gradient.move_stop(index as usize, position, self.gradient_cyclic);
				if Some(index) == self.active_marker_index {
					self.active_marker_index = Some(new_index as u32);
				}
			}
			SpectrumInputUpdate::MoveMidpoint { index, position } => {
				if (index as usize) >= gradient.len() {
					return;
				}
				gradient.set_midpoint(index as usize, position.clamp(MIN_MIDPOINT, MAX_MIDPOINT));
			}
			SpectrumInputUpdate::InsertMarker { position } => {
				let new_index = gradient.insert_stop(position, self.gradient_settings());
				self.active_marker_index = Some(new_index as u32);
				self.active_marker_is_midpoint = false;
				if let Some(color) = gradient.color(new_index) {
					self.adopt_color(color);
					self.snapshot_old();
				}
			}
			SpectrumInputUpdate::InsertDuplicate { index, position } => {
				let source = index as usize;
				let Some(insert_index) = gradient.duplicate_stop(source, position, self.gradient_cyclic) else {
					return;
				};
				// The dragged stop (the duplication source) stays active. Its index shifts up if the frozen copy landed at or before it.
				let dragged_index = if insert_index <= source { source + 1 } else { source };
				self.active_marker_index = Some(dragged_index as u32);
				self.active_marker_is_midpoint = false;
			}
			SpectrumInputUpdate::RemoveDuplicate { index } => {
				let anchor = index as usize;
				if anchor >= gradient.len() || gradient.len() <= 2 {
					return;
				}
				// Never remove the active (dragged) stop itself, this should only ever target the frozen copy.
				if self.active_marker_index == Some(anchor as u32) {
					return;
				}
				gradient.remove(anchor);
				// Keep the dragged stop active. Its index shifts down if the removed copy came before it.
				if let Some(active) = self.active_marker_index
					&& (anchor as u32) < active
				{
					self.active_marker_index = Some(active - 1);
				}
			}
			SpectrumInputUpdate::DeleteMarker { index } => {
				// Enforce minimum stop count. The gradient editor needs at least 2 stops to remain meaningful.
				if gradient.len() <= 2 || (index as usize) >= gradient.len() {
					return;
				}
				gradient.remove(index as usize);
				let new_active = (index as usize).min(gradient.len() - 1);
				self.active_marker_index = Some(new_active as u32);
				self.active_marker_is_midpoint = false;
				if let Some(color) = gradient.color(new_active) {
					self.adopt_color(color);
					self.snapshot_old();
				}
			}
			SpectrumInputUpdate::ResetMidpoint { index } => {
				gradient.reset_midpoint(index as usize);
			}
			SpectrumInputUpdate::ResetMarker { index } => {
				let i = index as usize;
				let count = gradient.len();
				if i >= count {
					return;
				}
				// Each stop's "natural" position is its evenly-spaced fraction along 0..1, e.g., for 5 stops: 0, 0.25, 0.5, 0.75, 1 (or fifths when cyclic, leaving the wrapped interval its share). Falls back to the midpoint between neighbors when the natural position would push the stop past another.
				let left = if i == 0 { 0. } else { gradient.position(i - 1, self.gradient_cyclic) };
				let right = if i + 1 < count { gradient.position(i + 1, self.gradient_cyclic) } else { 1. };
				let denominator = if self.gradient_cyclic { count } else { count - 1 };
				let natural = if count <= 1 { 0. } else { i as f64 / denominator as f64 };
				let new_position = if (left..=right).contains(&natural) { natural } else { (left + right) / 2. };
				let new_index = gradient.move_stop(i, new_position, self.gradient_cyclic);
				if Some(index) == self.active_marker_index {
					self.active_marker_index = Some(new_index as u32);
				}
			}
			SpectrumInputUpdate::ActiveMarker { .. } => unreachable!("handled above"),
		}

		responses.add(FrontendMessage::ColorPickerColorChanged {
			value: FillChoice::Gradient(GradientRamp {
				gradient_spread: self.gradient_spread,
				gradient_space: self.gradient_space,
				gradient_cyclic: self.gradient_cyclic,
				gradient_hue_direction: self.gradient_hue_direction,
				gradient_interpolation: self.gradient_interpolation,
				..GradientRamp::from(&gradient)
			}),
		});
		self.gradient = Some(gradient);
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
			let markers = (0..gradient.len())
				.filter_map(|i| Some(SpectrumMarker::new(gradient.position(i, self.gradient_cyclic), gradient.midpoint(i), gradient.color(i)?)))
				.collect();
			let mut row_widgets = vec![
				SpectrumInput::new(GradientStops::from(gradient))
					.track_space(self.gradient_space)
					.track_cyclic(self.gradient_cyclic)
					.track_hue_direction(self.gradient_hue_direction)
					.track_interpolation(self.gradient_interpolation)
					.markers(markers)
					.active_marker_index(self.active_marker_index)
					.active_marker_is_midpoint(self.active_marker_is_midpoint)
					.show_midpoints(true)
					.allow_insert(!self.disabled)
					.allow_delete(!self.disabled)
					.allow_reorder(true)
					.disabled(self.disabled)
					.on_update(|update: &SpectrumInputUpdate| ColorPickerMessage::GradientUpdate { update: update.clone() }.into())
					.widget_instance(),
			];

			if let Some(active) = self.active_marker_index {
				let active_index = active as usize;
				let position_value = match (self.active_marker_is_midpoint, active_index < gradient.len()) {
					(_, false) => 0.,
					(true, true) => gradient.midpoint(active_index),
					(false, true) => gradient.position(active_index, self.gradient_cyclic),
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
		// RGB readouts display sRGB byte values to the user, so we convert from linear-light to gamma here before quantizing.
		let rgb_255 = new_color.map(|c| {
			let [r, g, b, _] = c.to_gamma_srgb_channels();
			(r as f64 * 255., g as f64 * 255., b as f64 * 255.)
		});

		// Epsilon comparison since the picker round-trips through HSV
		let differs = match (new_color, old_color) {
			(Some(a), Some(b)) => {
				const EPSILON: f32 = 1e-6;
				(a.r() - b.r()).abs() >= EPSILON || (a.g() - b.g()).abs() >= EPSILON || (a.b() - b.b()).abs() >= EPSILON || (a.a() - b.a()).abs() >= EPSILON
			}
			(None, None) => false,
			_ => true,
		};
		let outline_amount = contrasting_outline_factor(new_color).max(contrasting_outline_factor(old_color));

		let mut groups = Vec::new();

		// New/old comparison swatch with swap button
		groups.push(LayoutGroup::row(vec![
			ColorComparisonInput::new(new_color.map(SRGBA8::from), old_color.map(SRGBA8::from))
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
				.tooltip_description("Also known as Hue/Saturation/Brightness (HSB), but distinct from Hue/Saturation/Lightness (HSL), a different color model.")
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

		// Gradient spread, trailed by the cyclic wrap toggle (only present when the picker is in gradient mode)
		if self.gradient.is_some() {
			let entries = RadioEntryData::list_from_choice_type(|gradient_spread| ColorPickerMessage::SetGradientSpread { gradient_spread }.into());

			groups.push(LayoutGroup::row(vec![
				TextLabel::new("Ends").tooltip_label("Gradient Spread / Cyclic").tooltip_description(ENDS_DESCRIPTION).widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				RadioInput::new(entries)
					.narrow(true)
					.selected_index(Some(self.gradient_spread as u32))
					.disabled(self.disabled)
					.widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				CheckboxInput::new(self.gradient_cyclic)
					.icon("Link")
					.disabled(self.disabled)
					.tooltip_label("Cyclic")
					.tooltip_description(CYCLIC_DESCRIPTION)
					.on_update(|checkbox_input: &CheckboxInput| {
						ColorPickerMessage::SetGradientCyclic {
							gradient_cyclic: checkbox_input.checked,
						}
						.into()
					})
					.widget_instance(),
			]));
		}

		// Gradient color space (only present when the picker is in gradient mode)
		if self.gradient.is_some() {
			let entries = MenuListEntry::sections_from_choice_type(|gradient_space| ColorPickerMessage::SetGradientSpace { gradient_space }.into());

			groups.push(LayoutGroup::row(vec![
				TextLabel::new("Space").tooltip_label("Gradient Space").tooltip_description(SPACE_DESCRIPTION).widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				DropdownInput::new(entries)
					.selected_index(Some(self.gradient_space as u32))
					.disabled(self.disabled)
					.tooltip_label("Gradient Space")
					.tooltip_description(SPACE_DESCRIPTION)
					.widget_instance(),
			]));
		}

		// Gradient interpolation (only present when the picker is in gradient mode)
		if self.gradient.is_some() {
			let entries = MenuListEntry::sections_from_choice_type(|gradient_interpolation| ColorPickerMessage::SetGradientInterpolation { gradient_interpolation }.into());

			groups.push(LayoutGroup::row(vec![
				TextLabel::new("Intrp.")
					.tooltip_label("Gradient Interpolation")
					.tooltip_description(INTERPOLATION_DESCRIPTION)
					.widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				DropdownInput::new(entries)
					.selected_index(Some(self.gradient_interpolation as u32))
					.disabled(self.disabled)
					.tooltip_label("Gradient Interpolation")
					.tooltip_description(INTERPOLATION_DESCRIPTION)
					.widget_instance(),
			]));
		}

		// Gradient hue direction (only present when the chosen space is polar)
		if self.gradient.is_some() && self.gradient_space.is_polar() {
			let entries = MenuListEntry::sections_from_choice_type(|gradient_hue_direction| ColorPickerMessage::SetGradientHueDirection { gradient_hue_direction }.into());

			groups.push(LayoutGroup::row(vec![
				TextLabel::new("Arc")
					.tooltip_label("Gradient Hue Direction")
					.tooltip_description(HUE_DIRECTION_DESCRIPTION)
					.widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				DropdownInput::new(entries)
					.selected_index(Some(self.gradient_hue_direction as u32))
					.disabled(self.disabled)
					.tooltip_label("Gradient Hue Direction")
					.tooltip_description(HUE_DIRECTION_DESCRIPTION)
					.widget_instance(),
			]));
		}

		// Color presets (None / Black / White / pure colors / eyedropper)
		groups.push(LayoutGroup::row(vec![
			ColorPresetsInput::default()
				.disabled(self.disabled)
				.show_none_option(self.allow_none && self.gradient.is_none())
				.on_update(|update: &ColorPresetsInputUpdate| match update {
					ColorPresetsInputUpdate::Preset(fill_choice) => ColorPickerMessage::PickPreset {
						preset: FillChoice::from(fill_choice),
					}
					.into(),
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
const ENDS_DESCRIPTION: &str = "The method for how the gradient continues beyond its ends, and whether its stops cycle back around from last to first.";
const CYCLIC_DESCRIPTION: &str = "Treats the stops as a cycle, interpolating from the last stop back around to the first.";
const SPACE_DESCRIPTION: &str = "The color space where stops interpolate toward their neighbors.";
const HUE_DIRECTION_DESCRIPTION: &str = "Which way around the hue wheel the stops interpolate.";
const INTERPOLATION_DESCRIPTION: &str = "The path the stops interpolate along, deciding whether the gradient jumps, turns corners, or flows smoothly through them.";

/// The popover's background color as sRGB gamma-encoded channels (the `--color-2-mildblack` design token, `#222`).
/// Used by the comparison swatch's outline computation to brighten the inset border for colors close to this background.
const POPOVER_BACKGROUND_GAMMA_CHANNELS: [f32; 4] = [0x22 as f32 / 255., 0x22 as f32 / 255., 0x22 as f32 / 255., 1.];
/// The luminance window within which a color is considered close enough to the popover background
/// to warrant an outline. Mirrors the `proximityRange` argument the legacy frontend passed to `contrastingOutlineFactor`.
const OUTLINE_PROXIMITY_RANGE: f64 = 0.01;

/// Returns a 0..1 outline strength for the comparison swatch, growing toward 1 as the color's luminance and saturation
/// both approach the popover background's luminance, when a color would otherwise visually blend into the popover.
fn contrasting_outline_factor(color: Option<Color>) -> f64 {
	let Some(color) = color else { return 0. };

	// WCAG-style relative luminance, with alpha composited over white in sRGB gamma space (matching the perceptual intent of `SRGBA8::contrasting_text_color`).
	let luminance_from_gamma_channels = |[r, g, b, a]: [f32; 4]| -> f64 {
		let inv_a = 1. - a;
		Color::from_gamma_srgb_channels(inv_a + r * a, inv_a + g * a, inv_a + b * a, 1.).luminance_rec_709() as f64
	};

	let color_gamma_channels = color.to_gamma_srgb_channels();
	let distance = (luminance_from_gamma_channels(POPOVER_BACKGROUND_GAMMA_CHANNELS) - luminance_from_gamma_channels(color_gamma_channels))
		.abs()
		.max(0.);
	let proximity = 1. - (distance / OUTLINE_PROXIMITY_RANGE).min(1.);
	let [_, saturation, _, _] = color.to_hsva();
	proximity * (1. - saturation as f64)
}

/// Format a linear `Color` as a `#`-prefixed hex string, including the alpha component only if it's not fully opaque.
fn color_to_hex_optional_alpha(color: &Color) -> String {
	SRGBA8::from(*color).to_css_hex()
}
