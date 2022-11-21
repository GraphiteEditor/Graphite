use super::utility_types::TransformOp;
use crate::messages::layout::utility_types::widgets::assist_widgets::PivotPosition;
use crate::messages::portfolio::document::utility_types::misc::TargetDocument;
use crate::messages::prelude::*;

use graphene::layers::imaginate_layer::{ImaginateMaskFillContent, ImaginateMaskPaintMode, ImaginateSamplingMethod};
use graphene::layers::style::{Fill, Stroke};
use graphene::LayerId;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, PropertiesPanel)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PropertiesPanelMessage {
	// Messages
	CheckSelectedWasDeleted { path: Vec<LayerId> },
	CheckSelectedWasUpdated { path: Vec<LayerId> },
	ClearSelection,
	Deactivate,
	Init,
	ModifyFill { fill: Fill },
	ModifyFont { font_family: String, font_style: String, size: f64 },
	ModifyName { name: String },
	ModifyStroke { stroke: Stroke },
	ModifyText { new_text: String },
	ModifyTransform { value: f64, transform_op: TransformOp },
	ResendActiveProperties,
	SetActiveLayers { paths: Vec<Vec<LayerId>>, document: TargetDocument },
	SetImaginateCfgScale { cfg_scale: f64 },
	SetImaginateDenoisingStrength { denoising_strength: f64 },
	SetImaginateLayerPath { layer_path: Option<Vec<LayerId>> },
	SetImaginateMaskBlurPx { mask_blur_px: u32 },
	SetImaginateMaskFillContent { mode: ImaginateMaskFillContent },
	SetImaginateMaskPaintMode { paint: ImaginateMaskPaintMode },
	SetImaginateNegativePrompt { negative_prompt: String },
	SetImaginatePrompt { prompt: String },
	SetImaginateRestoreFaces { restore_faces: bool },
	SetImaginateSamples { samples: u32 },
	SetImaginateSamplingMethod { method: ImaginateSamplingMethod },
	SetImaginateScaleFromResolution,
	SetImaginateSeed { seed: u64 },
	SetImaginateSeedRandomize,
	SetImaginateSeedRandomizeAndGenerate,
	SetImaginateTiling { tiling: bool },
	SetImaginateUseImg2Img { use_img2img: bool },
	SetPivot { new_position: PivotPosition },
	UpdateSelectedDocumentProperties,
}
