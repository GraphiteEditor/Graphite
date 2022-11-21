use super::utility_functions::{register_artboard_layer_properties, register_artwork_layer_properties};
use super::utility_types::PropertiesPanelMessageHandlerData;
use crate::application::generate_uuid;
use crate::messages::layout::utility_types::layout_widget::{Layout, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::portfolio::document::properties_panel::utility_functions::apply_transform_operation;
use crate::messages::portfolio::document::utility_types::misc::TargetDocument;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;

use graphene::{LayerId, Operation};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PropertiesPanelMessageHandler {
	active_selection: Option<(Vec<LayerId>, TargetDocument)>,
}

impl<'a> MessageHandler<PropertiesPanelMessage, (&PersistentData, PropertiesPanelMessageHandlerData<'a>)> for PropertiesPanelMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: PropertiesPanelMessage, (persistent_data, data): (&PersistentData, PropertiesPanelMessageHandlerData), responses: &mut VecDeque<Message>) {
		let PropertiesPanelMessageHandlerData {
			artwork_document,
			artboard_document,
			selected_layers,
			node_graph_message_handler,
		} = data;
		let get_document = |document_selector: TargetDocument| match document_selector {
			TargetDocument::Artboard => artboard_document,
			TargetDocument::Artwork => artwork_document,
		};
		use PropertiesPanelMessage::*;
		match message {
			SetActiveLayers { paths, document } => {
				if paths.len() != 1 {
					// TODO: Allow for multiple selected layers
					responses.push_back(PropertiesPanelMessage::ClearSelection.into());
					responses.push_back(NodeGraphMessage::CloseNodeGraph.into());
				} else {
					let path = paths.into_iter().next().unwrap();
					if Some((path.clone(), document)) != self.active_selection {
						self.active_selection = Some((path, document));
						responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
						responses.push_back(NodeGraphMessage::CloseNodeGraph.into());
					}
				}
			}
			ClearSelection => {
				responses.push_back(
					LayoutMessage::SendLayout {
						layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
						layout_target: LayoutTarget::PropertiesOptions,
					}
					.into(),
				);
				responses.push_back(
					LayoutMessage::SendLayout {
						layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
						layout_target: LayoutTarget::PropertiesSections,
					}
					.into(),
				);
				self.active_selection = None;
			}
			Deactivate => responses.push_back(
				BroadcastMessage::UnsubscribeEvent {
					on: BroadcastEvent::SelectionChanged,
					message: Box::new(PropertiesPanelMessage::UpdateSelectedDocumentProperties.into()),
				}
				.into(),
			),
			Init => responses.push_back(
				BroadcastMessage::SubscribeEvent {
					on: BroadcastEvent::SelectionChanged,
					send: Box::new(PropertiesPanelMessage::UpdateSelectedDocumentProperties.into()),
				}
				.into(),
			),
			ModifyFont { font_family, font_style, size } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");

				responses.push_back(self.create_document_operation(Operation::ModifyFont { path, font_family, font_style, size }));
				responses.push_back(ResendActiveProperties.into());
			}
			ModifyTransform { value, transform_op } => {
				let (path, target_document) = self.active_selection.as_ref().expect("Received update for properties panel with no active layer");
				let layer = get_document(*target_document).layer(path).unwrap();

				let transform = apply_transform_operation(layer, transform_op, value, &persistent_data.font_cache);

				responses.push_back(self.create_document_operation(Operation::SetLayerTransform { path: path.clone(), transform }));
			}
			ModifyName { name } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(self.create_document_operation(Operation::SetLayerName { path, name }))
			}
			ModifyFill { fill } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(self.create_document_operation(Operation::SetLayerFill { path, fill }));
			}
			ModifyStroke { stroke } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(self.create_document_operation(Operation::SetLayerStroke { path, stroke }))
			}
			ModifyText { new_text } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::SetTextContent { path, new_text }.into())
			}
			SetPivot { new_position } => {
				let (layer_path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				let position: Option<glam::DVec2> = new_position.into();
				let pivot = position.unwrap().into();

				responses.push_back(Operation::SetPivot { layer_path, pivot }.into());
			}
			CheckSelectedWasUpdated { path } => {
				if self.matches_selected(&path) {
					responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into())
				}
			}
			CheckSelectedWasDeleted { path } => {
				if self.matches_selected(&path) {
					self.active_selection = None;
					responses.push_back(
						LayoutMessage::SendLayout {
							layout_target: LayoutTarget::PropertiesOptions,
							layout: Layout::WidgetLayout(WidgetLayout::default()),
						}
						.into(),
					);
					responses.push_back(
						LayoutMessage::SendLayout {
							layout_target: LayoutTarget::PropertiesSections,
							layout: Layout::WidgetLayout(WidgetLayout::default()),
						}
						.into(),
					);
				}
			}
			ResendActiveProperties => {
				if let Some((path, target_document)) = self.active_selection.clone() {
					let document = get_document(target_document);
					let layer = document.layer(&path).unwrap();
					match target_document {
						TargetDocument::Artboard => register_artboard_layer_properties(layer, responses, persistent_data),
						TargetDocument::Artwork => register_artwork_layer_properties(document, path, layer, responses, persistent_data, node_graph_message_handler),
					}
				}
			}
			UpdateSelectedDocumentProperties => responses.push_back(
				PropertiesPanelMessage::SetActiveLayers {
					paths: selected_layers.map(|path| path.to_vec()).collect(),
					document: TargetDocument::Artwork,
				}
				.into(),
			),
			SetImaginatePrompt { prompt } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetPrompt { path, prompt }.into());
			}
			SetImaginateNegativePrompt { negative_prompt } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetNegativePrompt { path, negative_prompt }.into());
			}
			SetImaginateDenoisingStrength { denoising_strength } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetDenoisingStrength { path, denoising_strength }.into());
			}
			SetImaginateLayerPath { layer_path } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetLayerPath { path, layer_path }.into());
			}
			SetImaginateMaskBlurPx { mask_blur_px } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetMaskBlurPx { path, mask_blur_px }.into());
			}
			SetImaginateMaskFillContent { mode } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetMaskFillContent { path, mode }.into());
			}
			SetImaginateMaskPaintMode { paint } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetMaskPaintMode { path, paint }.into());
			}
			SetImaginateSamples { samples } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetSamples { path, samples }.into());
			}
			SetImaginateSamplingMethod { method } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::SetImaginateSamplingMethod { path, method }.into());
			}
			SetImaginateScaleFromResolution => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");

				responses.push_back(Operation::ImaginateSetScaleFromResolution { path }.into());
			}
			SetImaginateSeed { seed } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetSeed { path, seed }.into());
			}
			SetImaginateSeedRandomize => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				let seed = generate_uuid();
				responses.push_back(Operation::ImaginateSetSeed { path, seed }.into());
			}
			SetImaginateSeedRandomizeAndGenerate => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				let seed = generate_uuid();
				responses.push_back(Operation::ImaginateSetSeed { path, seed }.into());
				responses.push_back(DocumentMessage::ImaginateGenerate.into());
			}
			SetImaginateCfgScale { cfg_scale } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetCfgScale { path, cfg_scale }.into());
			}
			SetImaginateUseImg2Img { use_img2img } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetUseImg2Img { path, use_img2img }.into());
			}
			SetImaginateRestoreFaces { restore_faces } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetRestoreFaces { path, restore_faces }.into());
			}
			SetImaginateTiling { tiling } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::ImaginateSetTiling { path, tiling }.into());
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PropertiesMessageDiscriminant;)
	}
}

impl PropertiesPanelMessageHandler {
	fn matches_selected(&self, path: &[LayerId]) -> bool {
		let last_active_path_id = self.active_selection.as_ref().and_then(|(v, _)| v.last().copied());
		let last_modified = path.last().copied();
		matches!((last_active_path_id, last_modified), (Some(active_last), Some(modified_last)) if active_last == modified_last)
	}

	fn create_document_operation(&self, operation: Operation) -> Message {
		let (_, target_document) = self.active_selection.as_ref().unwrap();
		match *target_document {
			TargetDocument::Artboard => ArtboardMessage::DispatchOperation(Box::new(operation)).into(),
			TargetDocument::Artwork => DocumentMessage::DispatchOperation(Box::new(operation)).into(),
		}
	}
}
