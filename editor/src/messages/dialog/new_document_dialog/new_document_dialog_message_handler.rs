use crate::messages::dialog::document_setup_dialog::resolution_row;
use crate::messages::layout::layout_message_handler::unit_suffix;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use glam::DVec2;
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::units::{DEFAULT_PIXELS_PER_INCH, LengthUnit, TypedLength};

/// A dialog to allow users to set some initial options about a new document.
#[derive(Debug, Clone, ExtractField)]
pub struct NewDocumentDialogMessageHandler {
	pub name: String,
	pub infinite: bool,
	pub width: TypedLength,
	pub height: TypedLength,
	pub pixels_per_inch: f64,
}

impl Default for NewDocumentDialogMessageHandler {
	fn default() -> Self {
		Self {
			name: String::new(),
			infinite: false,
			width: TypedLength::default(),
			height: TypedLength::default(),
			pixels_per_inch: DEFAULT_PIXELS_PER_INCH,
		}
	}
}

#[message_handler_data]
impl MessageHandler<NewDocumentDialogMessage, ()> for NewDocumentDialogMessageHandler {
	fn process_message(&mut self, message: NewDocumentDialogMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			NewDocumentDialogMessage::Name { name } => self.name = name,
			NewDocumentDialogMessage::Infinite { infinite } => self.infinite = infinite,
			NewDocumentDialogMessage::DimensionsX { width } => {
				self.width = whole_pixels(width);
				self.height = self.in_unit(self.height, self.width.unit);
			}
			NewDocumentDialogMessage::DimensionsY { height } => {
				self.height = whole_pixels(height);
				self.width = self.in_unit(self.width, self.height.unit);
			}
			NewDocumentDialogMessage::Resolution { pixels_per_inch } => self.pixels_per_inch = pixels_per_inch,
			NewDocumentDialogMessage::Submit => {
				responses.add(PortfolioMessage::NewDocumentWithName { name: self.name.clone() });

				// Like the artboard, the resolution is part of the document's starting state rather than an undo step
				responses.add(DocumentMessage::SetPixelsPerInch {
					pixels_per_inch: self.pixels_per_inch,
				});

				let dimensions = self.artboard_dimensions();
				if self.infinite {
					// Infinite canvas: add a locked white background layer
					graph_modification_utils::new_background_layer(0, responses);
				} else if dimensions.x > 0. && dimensions.y > 0. {
					// Finite canvas: create an artboard with the specified dimensions
					responses.add(GraphOperationMessage::NewArtboard {
						id: NodeId::new(),
						location: DVec2::ZERO,
						dimensions,
						background: Color::WHITE,
						clip: true,
					});
					responses.add(NavigationMessage::CanvasPan { delta: dimensions });
				}

				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(ViewportMessage::RepropagateUpdate);

				responses.add(DocumentMessage::DeselectAllLayers);

				responses.add(DeferMessage::AfterNavigationReady {
					messages: vec![DocumentMessage::ZoomCanvasToFitAll.into(), PortfolioMessage::AutoSaveActiveDocument.into()],
				});

				responses.add(DocumentMessage::MarkAsSaved);
			}
		}

		self.send_dialog_to_frontend(responses);
	}

	advertise_actions!(NewDocumentDialogUpdate;
	);
}

impl DialogLayoutHolder for NewDocumentDialogMessageHandler {
	const ICON: &'static str = "File";
	const TITLE: &'static str = "New Document";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![
			TextButton::new("OK")
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseAndThen {
						followups: vec![NewDocumentDialogMessage::Submit.into()],
					}
					.into()
				})
				.widget_instance(),
			TextButton::new("Cancel").on_update(|_| DialogMessage::Close.into()).widget_instance(),
		];

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

impl LayoutHolder for NewDocumentDialogMessageHandler {
	fn layout(&self) -> Layout {
		let name = vec![
			TextLabel::new("Name").table_align(true).sizing(WidgetSizing::fixed(90)).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(&self.name)
				.on_update(|text_input: &TextInput| NewDocumentDialogMessage::Name { name: text_input.value.clone() }.into())
				// Matches the 100px of both NumberInputs below plus the 4px of the Related-type separator between them
				.sizing(WidgetSizing::default().min_width(204))
				.widget_instance(),
		];

		let checkbox_id = CheckboxId::new();
		let infinite = vec![
			TextLabel::new("Infinite Canvas")
				.table_align(true)
				.sizing(WidgetSizing::fixed(90))
				.for_checkbox(checkbox_id)
				.widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			CheckboxInput::new(self.infinite)
				.on_update(|checkbox_input: &CheckboxInput| NewDocumentDialogMessage::Infinite { infinite: checkbox_input.checked }.into())
				.for_label(checkbox_id)
				.widget_instance(),
		];

		// Each dimension shows the unit it was typed in and keeps any length unit typed into it
		let dimension_field = |length: TypedLength, label: &str| {
			NumberInput::new(Some(length.value))
				.label(label)
				.unit(unit_suffix(length.unit.symbol()))
				.kept_units(LengthUnit::ALL.iter().map(|unit| unit.symbol().to_string()).collect())
				.pixels_per_inch(self.pixels_per_inch)
				.min(0.)
				.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
				.disabled(self.infinite)
				.sizing(WidgetSizing::default().min_width(100))
		};
		let dimensions = vec![
			TextLabel::new("Dimensions").table_align(true).sizing(WidgetSizing::fixed(90)).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			dimension_field(self.width, "W")
				.on_update(|number_input: &NumberInput| NewDocumentDialogMessage::DimensionsX { width: typed_length(number_input) }.into())
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			dimension_field(self.height, "H")
				.on_update(|number_input: &NumberInput| NewDocumentDialogMessage::DimensionsY { height: typed_length(number_input) }.into())
				.widget_instance(),
		];

		let resolution = resolution_row(self.pixels_per_inch, |pixels_per_inch| NewDocumentDialogMessage::Resolution { pixels_per_inch }.into());

		Layout(vec![LayoutGroup::row(name), LayoutGroup::row(infinite), LayoutGroup::row(dimensions), LayoutGroup::row(resolution)])
	}
}

impl NewDocumentDialogMessageHandler {
	/// A dimension converted into `unit` at the chosen resolution, keeping its size.
	fn in_unit(&self, length: TypedLength, unit: LengthUnit) -> TypedLength {
		whole_pixels(TypedLength {
			value: length.unit.convert(length.value, unit, self.pixels_per_inch),
			unit,
		})
	}

	/// The artboard's size in whole pixels, converting a dimension typed in a physical unit at the chosen resolution.
	pub fn artboard_dimensions(&self) -> DVec2 {
		let pixels = |length: TypedLength| length.to_length(self.pixels_per_inch).pixels().round();
		DVec2::new(pixels(self.width), pixels(self.height))
	}
}

/// The length a dimension field holds, in the unit it shows.
fn typed_length(number_input: &NumberInput) -> TypedLength {
	let unit = math_parser::units::Unit::named(number_input.unit.trim()).and_then(LengthUnit::from_unit).unwrap_or_default();
	TypedLength {
		value: number_input.value.unwrap_or_default(),
		unit,
	}
}

/// Rounds a dimension typed in pixels to a whole number of them, leaving one in a physical unit as typed.
fn whole_pixels(length: TypedLength) -> TypedLength {
	if length.unit == LengthUnit::Pixel { TypedLength::pixels(length.value.round()) } else { length }
}

#[cfg(test)]
mod tests {
	use super::NewDocumentDialogMessageHandler;
	use crate::messages::layout::utility_types::widget_prelude::*;
	use crate::messages::portfolio::document::node_graph::document_node_definitions::ARTBOARD_DIMENSIONS_INPUT_INDEX;
	use crate::test_utils::test_prelude::*;
	use graph_craft::document::value::TaggedValue;
	use std::collections::HashMap;

	/// Commits text into the dialog's number field with this label or tooltip label, as the frontend sends a typed entry.
	fn type_into_field(editor: &mut EditorTestUtils, label: &str, text: &str) {
		// A widget's ID follows from its place in the layout, which the dialog's state doesn't change
		let mut layout = NewDocumentDialogMessageHandler::default().layout();
		layout.replace_widget_ids(LayoutTarget::DialogColumn1, &mut Vec::new(), &HashMap::new());
		let widget_id = layout.iter().find_map(|instance| match &*instance.widget {
			Widget::NumberInput(number_input) if number_input.label == label || number_input.tooltip_label == label => Some(instance.widget_id),
			_ => None,
		});
		let Some(widget_id) = widget_id else {
			panic!("the dialog has no number field labeled \"{label}\"")
		};

		editor.dispatch(LayoutMessage::WidgetValueUpdate {
			layout_target: LayoutTarget::DialogColumn1,
			widget_id,
			value: serde_json::Value::String(text.into()),
		});
	}

	async fn create_document(editor: &mut EditorTestUtils) {
		editor
			.handle_message(DialogMessage::CloseAndThen {
				followups: vec![NewDocumentDialogMessage::Submit.into()],
			})
			.await;
	}

	fn artboard_dimensions(editor: &EditorTestUtils) -> Option<DVec2> {
		let network_interface = &editor.active_document().network_interface;
		let artboard = network_interface.all_artboards().into_iter().next()?;
		let node = network_interface.document_network().nodes.get(&artboard.to_node())?;

		match node.inputs.get(ARTBOARD_DIMENSIONS_INPUT_INDEX)?.as_value()? {
			TaggedValue::DVec2(dimensions) => Some(*dimensions),
			_ => None,
		}
	}

	#[tokio::test]
	async fn a_new_document_starts_at_the_chosen_resolution_without_an_undo_step() {
		let mut editor = EditorTestUtils::create();
		editor.dispatch(DialogMessage::RequestNewDocumentDialog);
		type_into_field(&mut editor, "Resolution", "300");
		create_document(&mut editor).await;
		assert_eq!(editor.active_document().network_interface.pixels_per_inch(), 300.);
		assert_eq!(artboard_dimensions(&editor), Some(DVec2::new(1920., 1080.)));

		editor.handle_message(DocumentMessage::Undo).await;
		assert_eq!(editor.active_document().network_interface.pixels_per_inch(), 300.);
		assert_eq!(artboard_dimensions(&editor), Some(DVec2::new(1920., 1080.)));
	}

	#[tokio::test]
	async fn typing_a_unit_into_one_dimension_converts_the_other_into_it() {
		let mut editor = EditorTestUtils::create();
		editor.dispatch(DialogMessage::RequestNewDocumentDialog);
		type_into_field(&mut editor, "W", "210mm");

		// The height of 1080 px at 96 PPI became 285.75 mm, so it grows with the resolution like the width
		type_into_field(&mut editor, "Resolution", "300");
		create_document(&mut editor).await;
		assert_eq!(artboard_dimensions(&editor), Some(DVec2::new(2480., 3375.)));
	}

	#[tokio::test]
	async fn dimensions_typed_in_millimeters_convert_to_pixels_at_the_chosen_resolution() {
		let mut editor = EditorTestUtils::create();
		editor.dispatch(DialogMessage::RequestNewDocumentDialog);
		type_into_field(&mut editor, "W", "210mm");
		type_into_field(&mut editor, "H", "297mm");

		// The lengths stay in millimeters, so their size in pixels follows a resolution chosen after them
		type_into_field(&mut editor, "Resolution", "300");
		create_document(&mut editor).await;
		assert_eq!(artboard_dimensions(&editor), Some(DVec2::new(2480., 3508.)));
	}
}
