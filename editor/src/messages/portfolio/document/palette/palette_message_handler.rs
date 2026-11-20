use std::path::PathBuf;

use crate::application::generate_uuid;
use graphene_std::vector::style::FillChoice;
use graphene_std::{Color, color::SRGBA8};

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::palette::PaletteMessage;
use crate::messages::prelude::*;
use serde::{Deserialize, Serialize};

// TODO: Ig this should be imported from some preloaded assets file after the
// implementation of import from file feature.
pub const DEFAULT_PALETTES: [(&str, u32, &[SRGBA8]); 3] = [
	(
		"Warm Landscape",
		5,
		&[
			SRGBA8::new_opaque(0x26, 0x46, 0x53),
			SRGBA8::new_opaque(0x2A, 0x9D, 0x8F),
			SRGBA8::new_opaque(0xE9, 0xC4, 0x6A),
			SRGBA8::new_opaque(0xF4, 0xA2, 0x61),
			SRGBA8::new_opaque(0xE7, 0x6F, 0x51),
			SRGBA8::new_opaque(0xD6, 0x28, 0x28),
			SRGBA8::new_opaque(0x00, 0x30, 0x49),
			SRGBA8::new_opaque(0xF7, 0x7F, 0x00),
			SRGBA8::new_opaque(0xFC, 0xBF, 0x49),
			SRGBA8::new_opaque(0xEA, 0xE2, 0xB7),
		],
	),
	(
		"Nordic Frost",
		3,
		&[
			SRGBA8::new_opaque(0x2E, 0x34, 0x40),
			SRGBA8::new_opaque(0x3B, 0x42, 0x52),
			SRGBA8::new_opaque(0x43, 0x4C, 0x5E),
			SRGBA8::new_opaque(0x4C, 0x56, 0x6A),
			SRGBA8::new_opaque(0xD8, 0xDE, 0xE9),
			SRGBA8::new_opaque(0xE5, 0xE9, 0xF0),
			SRGBA8::new_opaque(0xEC, 0xEF, 0xF4),
			SRGBA8::new_opaque(0x88, 0xC0, 0xD0),
			SRGBA8::new_opaque(0x81, 0xA1, 0xC1),
			SRGBA8::new_opaque(0x5E, 0x81, 0xAC),
		],
	),
	(
		"Something",
		2,
		&[
			SRGBA8::new_opaque(0x26, 0x46, 0x53),
			SRGBA8::new_opaque(0x2A, 0x9D, 0x8F),
			SRGBA8::new_opaque(0xE9, 0xC4, 0x6A),
			SRGBA8::new_opaque(0xF4, 0xA2, 0x61),
			SRGBA8::new_opaque(0xE7, 0x6F, 0x51),
			SRGBA8::new_opaque(0xD6, 0x28, 0x28),
			SRGBA8::new_opaque(0x00, 0x30, 0x49),
			SRGBA8::new_opaque(0xF7, 0x7F, 0x00),
			SRGBA8::new_opaque(0xFC, 0xBF, 0x49),
			SRGBA8::new_opaque(0xEA, 0xE2, 0xB7),
		],
	),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct PaletteId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct SwatchId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ColorSwatch {
	pub id: SwatchId,
	pub name: String,
	pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Palette {
	pub id: PaletteId,
	pub name: String,
	// Palette specification where it can be 0 or None in which case the UI will flow
	// the palettes accordingly
	pub columns: Option<u32>,
	pub grid: Vec<Option<SwatchId>>,
	pub swatches: HashMap<SwatchId, ColorSwatch>,
}

#[derive(ExtractField)]
pub struct PaletteMessageContext {
	pub palette_open: bool,
}

#[derive(Debug, Clone, ExtractField)]
pub struct PaletteMessageHandler {
	// At any moment in time at least one palette should be open in the palette section
	pub active_palette: PaletteId,
	pub order: Vec<PaletteId>,
	pub palettes: HashMap<PaletteId, Palette>,
}

impl Default for PaletteMessageHandler {
	fn default() -> Self {
		let mut palettes = HashMap::new();
		let mut order = Vec::new();
		for &(name, columns, colors) in &DEFAULT_PALETTES {
			let mut grid = Vec::new();
			let mut swatches = HashMap::new();
			for color in colors {
				let id = SwatchId { 0: generate_uuid() };
				grid.push(Some(id));
				swatches.insert(
					id,
					ColorSwatch {
						id,
						name: "Untitled Color".to_string(),
						color: (*color).into(),
					},
				);
			}
			let palette_id = PaletteId { 0: generate_uuid() };
			order.push(palette_id);
			palettes.insert(
				palette_id,
				Palette {
					id: palette_id,
					name: name.to_string(),
					columns: Some(columns),
					grid,
					swatches,
				},
			);
		}
		Self {
			active_palette: order[0],
			order,
			palettes,
		}
	}
}

#[message_handler_data]
impl MessageHandler<PaletteMessage, PaletteMessageContext> for PaletteMessageHandler {
	fn process_message(&mut self, message: PaletteMessage, responses: &mut VecDeque<Message>, context: PaletteMessageContext) {
		let PaletteMessageContext { palette_open } = context;
		match message {
			PaletteMessage::Refresh => {
				if !palette_open {
					return;
				}

				self.refresh_palette(responses);
			}
			PaletteMessage::ActivePaletteChanged { id } => {
				self.active_palette = PaletteId(id);
				responses.add(PaletteMessage::Refresh);
			}
			PaletteMessage::PaletteNameChange { id, name } => {
				let palette = self.palettes.get_mut(&PaletteId { 0: id }).unwrap();
				palette.name = name;
				responses.add(PaletteMessage::Refresh);
			}
			PaletteMessage::AddColor { color } => todo!(),
			PaletteMessage::DeleteColor { swatch_id } => todo!(),
			PaletteMessage::InsertPaletteGPL { palette_name, content } => {
				if let Err(err) = self.parse_gpl_palette(palette_name, content) {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Failed to load palette".into(),
						description: format!("Could not parse Gimp palete file: {err}"),
					});
				} else {
					responses.add(PaletteMessage::Refresh);
				}
			}
			PaletteMessage::InsertPaletteKPL { palette_name, palette_path, content } => self.parse_kpl_palette(palette_name, palette_path, content),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PaletteMessage;)
	}
}

impl PaletteMessageHandler {
	pub fn refresh_palette(&self, responses: &mut VecDeque<Message>) {
		// TODO: Add Conditional to load palette from the open document's interface
		// If none is present or it is a new document load the Default.
		if true {
			let mut widgets = Vec::new();

			for palette_id in self.order.iter() {
				let Palette { id: _, name, columns, grid, swatches } = self.palettes.get(palette_id).unwrap();
				let is_active = *palette_id == self.active_palette;
				let target_id = palette_id.0;

				let header_row = vec![
					TextLabel::new(name).bold(true).sizing(WidgetSizing::default().grow(true)).widget_instance(),
					TextInput::new(name.clone())
						.sizing(WidgetSizing::default().grow(true))
						.disabled(true)
						.on_update(move |input: &TextInput| {
							PaletteMessage::PaletteNameChange {
								id: target_id,
								name: input.value.clone(),
							}
							.into()
						})
						.widget_instance(),
					TextButton::new("")
						.flush(true)
						.sizing(WidgetSizing::fixed(24))
						.disabled(is_active)
						.on_commit(move |_| PaletteMessage::ActivePaletteChanged { id: target_id }.into())
						.widget_instance(),
				];
				widgets.push(LayoutGroup::row(header_row));

				if is_active {
					let mut table_rows = Vec::new();
					if let Some(columns) = columns {
						for chunk in grid.chunks(*columns as usize) {
							let mut row: Vec<_> = chunk
								.iter()
								.map(|&id| {
									if let Some(swatch_id) = id {
										let swatch = swatches.get(&swatch_id).unwrap();
										ColorInput::new(FillChoice::Solid(swatch.color.into()))
											.narrow(true)
											.tooltip_label(swatch.name.clone())
											.widget_instance()
									} else {
										ColorInput::new(FillChoice::None).narrow(true).widget_instance()
									}
								})
								.collect();
							row.extend((row.len()..*columns as usize).map(|_| ColorInput::new(FillChoice::None).narrow(true).widget_instance()));
							table_rows.push(row);
						}
					} else {
						//TODO: Manage the case when the column size is not determined
					}

					widgets.push(LayoutGroup::table(table_rows, true));
				}
			}
			let left_bottom_bar = LayoutGroup::row(vec![
				TextLabel::new("Hello").widget_instance(),
				IconButton::new("Add", 24)
					.tooltip_label("Add Palette")
					.on_update(|_| IngestMessage::ImportPalette.into())
					.widget_instance(),
			]);

			responses.add(LayoutMessage::SendLayout {
				layout: Layout(widgets),
				layout_target: LayoutTarget::Palette,
			});
			responses.add(LayoutMessage::SendLayout {
				layout: Layout(vec![left_bottom_bar]),
				layout_target: LayoutTarget::PalettePanelBottomLeftBar,
			});
		} else {
			// TODO: Add attached palettes instead
		}
	}

	// Since the specification doesn't specify the string to be purely utf8, this parsing
	// tries to respect the approach.
	fn parse_gpl_palette(&mut self, palette_name: Option<String>, content: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
		if content.is_empty() || content.len() < 12 {
			warn!("Failed to parse Gimp palette due to invalid format");
			return Err("Invalid format".into());
		}
		let mut lines = content.split(|b| *b == b'\n').peekable();

		let mut magic = lines.next().unwrap_or_default();
		magic = magic.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(magic);
		magic.trim_ascii_start();
		if !magic.starts_with(b"GIMP Palette") {
			warn!("Failed to parse Gimp palette due to invalid format");
			return Err("Invalid format".into());
		}

		let mut is_gpl_version_1 = true;
		let palette_id = PaletteId { 0: generate_uuid() };
		let mut palette = Palette {
			id: palette_id,
			name: String::new(),
			columns: None,
			grid: Vec::new(),
			swatches: HashMap::new(),
		};

		if let Some(x) = lines.peek()
			&& x.starts_with(b"Name:")
		{
			let line = lines.next().unwrap();
			is_gpl_version_1 = false;
			let idx = line.iter().position(|x| *x == b':').unwrap();
			let (_, name) = line.split_at(idx);
			palette.name = String::from_utf8(name[1..].to_vec())?;
		} else {
			palette.name = palette_name.unwrap_or("Untitled".to_string());
		}

		if let Some(x) = lines.peek()
			&& !is_gpl_version_1
			&& x.starts_with(b"Columns: ")
		{
			let line = lines.next().unwrap();
			let idx = line.iter().position(|x| *x == b':').unwrap();
			let (_, name) = line.split_at(idx);
			palette.columns = Some(std::str::from_utf8(&name[1..])?.trim().parse()?);
		}

		for line in lines {
			if line.is_empty() || line.trim_ascii_start().starts_with(b"#") {
				continue;
			}
			let line_str = std::str::from_utf8(line)?.trim();
			if line_str.is_empty() {
				continue;
			}

			let mut parts = line_str.split_whitespace();
			let red: u8 = parts.next().ok_or("Missing red")?.parse()?;
			let green: u8 = parts.next().ok_or("Missing green")?.parse()?;
			let blue: u8 = parts.next().ok_or("Missing blue")?.parse()?;
			let remaining_name = parts.collect::<Vec<_>>().join(" ");
			let name = if remaining_name.is_empty() { "Untitled Color".to_string() } else { remaining_name };

			let color = SRGBA8::new_opaque(red, green, blue);
			let id = SwatchId { 0: generate_uuid() };
			palette.grid.push(Some(id));
			palette.swatches.insert(id, ColorSwatch { id, name, color: color.into() });
		}

		self.order.push(palette_id);
		self.palettes.insert(palette_id, palette);
		self.active_palette = palette_id;

		return Ok(());
	}

	fn parse_kpl_palette(&mut self, palette_name: Option<String>, palette_path: Option<PathBuf>, content: Vec<u8>) {
		// TODO: Add a kpl parsing implementation
	}
}
