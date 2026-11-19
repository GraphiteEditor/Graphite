//! How an item's effects split into those flattened into the content's own image and the pieces drawn as layers onto what lies beneath the item, which every renderer follows.
//! Runs of pieces that blend normally merge with each other and with the content into one layer, which the item's opacity fades as a whole.

use crate::Graphic;
use crate::adjustment::Adjustment;
use crate::effects::{BevelStyle, EffectCapture, EffectOperation, Effects, GlowFill, OutlineFill, outline_extents};
use core_types::Color;
use core_types::blending::BlendMode;
use std::borrow::Cow;
use std::ops::Range;

/// Which side of the content a piece of an effect draws on, and how.
/// For PSD interop, an interior piece blends within the content's share of each pixel onto the content's color mixed with what lies beneath by the fill, so it splits by the fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PiecePlace {
	/// Beneath the content, like a drop shadow.
	Beneath,
	/// Over the content within its silhouette, like an inner shadow, unfaded by its fill: atop the content in the content's own layer, or cut to the content in a layer of its own.
	Within,
	/// The part of an interior piece blending with the content's own colors by its mode, in the content's own layer.
	/// With `over`, it takes only the strength left by its part over the content.
	IntoContent { over: bool },
	/// The part of an interior piece blending with what shows through the content's fill, in a layer of its mode beneath the content's, or with `over`, over it.
	Through { over: bool },
}

/// One coverage of an effect that draws as a piece of its own, named like the map of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectPiece {
	pub capture: EffectCapture,
	pub place: PiecePlace,
	/// The mode the piece blends by, given as normal where its solid color blends the same either way.
	pub blend_mode: BlendMode,
	/// Whether the content hides the piece where it covers it, as for a shadow beneath the content with knockout on.
	pub knocks_out: bool,
}

/// What a layer of an item draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerPart {
	/// The content with its flattened effects carried out, faded by its fill.
	Content,
	Piece(EffectPiece),
}

/// One layer an item draws onto what lies beneath it: its parts composited normally in order, then blended by its mode and faded by the item's opacity.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectLayer {
	pub blend_mode: BlendMode,
	pub parts: Vec<LayerPart>,
}

impl EffectLayer {
	/// Whether this layer draws the content itself.
	pub fn holds_content(&self) -> bool {
		self.parts.contains(&LayerPart::Content)
	}
}

/// How an item with effects draws: the leading effects carried out within the content's own image, then its layers from the bottom up.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectLayers {
	/// How many of the effects, from the first, are carried out within the content's image.
	pub flattened: usize,
	pub layers: Vec<EffectLayer>,
	/// The rows from the first mask after every other effect to the end, which fade each finished layer at each pixel like the item's opacity. Empty where those masks flatten what's before them instead.
	pub trailing_masks: Range<usize>,
}

impl EffectLayers {
	/// The layers of content drawn with `blend_mode` and faded by `fill` through `effects`, or with every effect flattened if not `layered`, as for content clipped by a mask.
	/// Content in a mode CSS lacks is drawn normally, so it's planned that way.
	pub fn plan(effects: &Effects, blend_mode: BlendMode, fill: f32, layered: bool) -> Self {
		let operations: Vec<EffectOperation> = effects.operations().collect();
		let blend_mode = if blend_mode.to_svg_style_name().is_some() { blend_mode } else { BlendMode::Normal };

		// Normal content at full fill hides what lies beneath its share, so its interior pieces blend with it alone, in any mode, within its own layer
		let shares_content = blend_mode == BlendMode::Normal;
		let blends_into_content = shares_content && fill >= 1.;

		// The masks after every other effect fade the layers rather than the content's image, so they flatten nothing before them
		let no_rows = operations.len()..operations.len();
		let mut trailing_masks = if layered { trailing_masks(&operations) } else { no_rows.clone() };

		// Rows that change nothing, like a blur of no radius, flatten nothing before them
		let mut flattened = match layered {
			true => (operations[..trailing_masks.start].iter())
				.rposition(|operation| !operation.changes_nothing() && !can_be_layered(operation, blends_into_content))
				.map_or(0, |index| index + 1),
			false => operations.len(),
		};

		// Each piece with the mode it blends by, in the order of the rows drawing them
		let pieces_from = |start: usize, end: usize| -> Vec<(EffectPiece, BlendMode)> {
			(operations.iter().enumerate().take(end).skip(start))
				.filter(|(_, operation)| !operation.changes_nothing())
				.flat_map(|(index, operation)| pieces(index, operation))
				.collect()
		};
		let has_beneath = |pieces: &[(EffectPiece, BlendMode)]| pieces.iter().any(|(piece, _)| piece.place == PiecePlace::Beneath);

		// Flattened shadows and glows that no later pixel filter merges into the content would draw over the layered ones, so then everything is flattened
		let last_pixel_filter = merged_into_content(&operations[..flattened]);
		let unmerged = pieces_from(last_pixel_filter, flattened);
		let mut pieces = pieces_from(flattened, operations.len());
		if has_beneath(&unmerged) && has_beneath(&pieces) {
			flattened = operations.len();
			trailing_masks = no_rows.clone();
			pieces.clear();
		}

		let mut parts: Vec<(LayerPart, BlendMode)> = (pieces.iter())
			.filter(|(piece, _)| piece.place == PiecePlace::Beneath)
			.map(|&(piece, mode)| (LayerPart::Piece(piece), mode))
			.chain([(LayerPart::Content, blend_mode)])
			.collect();

		// A shadow not knocked out in the content's layer, flattened or layered, would hide a part placed beneath that layer
		let shows_through = |piece: &EffectPiece| piece.place == PiecePlace::Beneath && !piece.knocks_out;
		let mut content_run = parts.iter().rev().skip(1).take_while(|(_, mode)| *mode == BlendMode::Normal);
		let covered_beneath = content_run.any(|(part, _)| matches!(part, LayerPart::Piece(piece) if shows_through(piece))) || unmerged.iter().any(|(piece, _)| shows_through(piece));

		// Interior pieces of normal content blend within its share, in its layer. Below a full fill, a blended one splits off its part through the fill into a layer beneath
		// the content's, or over it when its mode isn't straight along what lies beneath or a shadow covers that, as every later piece then does to keep their order.
		let mut into_content = Vec::new();
		let mut through_beneath = Vec::new();
		let mut over = false;
		for &(piece, mode) in pieces.iter().filter(|(piece, _)| piece.place == PiecePlace::Within) {
			let part = |place: PiecePlace| EffectPiece { place, ..piece };
			if !shares_content {
				parts.push((LayerPart::Piece(piece), mode));
				continue;
			}

			over |= !blends_into_content && mode != BlendMode::Normal && (covered_beneath || !is_straight_along_backdrop(mode));
			match (blends_into_content || !over, mode == BlendMode::Normal) {
				(true, true) => into_content.push(piece),
				(true, false) if blends_into_content => into_content.push(part(PiecePlace::IntoContent { over: false })),
				(true, false) => {
					into_content.push(part(PiecePlace::IntoContent { over: false }));
					through_beneath.push(part(PiecePlace::Through { over: false }));
				}
				(false, _) => {
					into_content.push(part(PiecePlace::IntoContent { over: true }));
					parts.push((LayerPart::Piece(part(PiecePlace::Through { over: true })), mode));
				}
			}
		}

		// Neighboring parts that blend normally composite together into one layer
		let mut layers: Vec<EffectLayer> = Vec::new();
		for (part, blend_mode) in parts {
			match layers.last_mut() {
				Some(layer) if blend_mode == BlendMode::Normal && layer.blend_mode == BlendMode::Normal => layer.parts.push(part),
				_ => layers.push(EffectLayer { blend_mode, parts: vec![part] }),
			}
		}

		// The parts blending with what shows through the faded content lie directly beneath the normal run holding the content, which still fades as one
		if let Some(content_layer) = layers.iter().position(EffectLayer::holds_content) {
			layers[content_layer].parts.extend(into_content.into_iter().map(LayerPart::Piece));
			let through = through_beneath.into_iter().map(|piece| EffectLayer {
				blend_mode: piece.blend_mode,
				parts: vec![LayerPart::Piece(piece)],
			});
			layers.splice(content_layer..content_layer, through);
		}

		// One normal layer at a full fill draws the same with all its effects flattened into the content's image, which takes fewer passes
		if blends_into_content && matches!(layers.as_slice(), [layer] if layer.blend_mode == BlendMode::Normal) {
			let content = EffectLayer {
				blend_mode: BlendMode::Normal,
				parts: vec![LayerPart::Content],
			};
			return Self {
				flattened: operations.len(),
				layers: vec![content],
				trailing_masks: no_rows,
			};
		}

		Self { flattened, layers, trailing_masks }
	}

	/// Like [`Self::plan`], for effects stamped in more than one space (see [`Effects::runs`]), as renderers draw them.
	/// The runs before the last are flattened into its content, faded by the `fill`, so only the last run's effects can be layered, named by their places in the whole list.
	pub fn plan_runs(effects: &Effects, blend_mode: BlendMode, fill: f32, layered: bool) -> Self {
		let runs = effects.runs();
		let Some((_, last_run)) = runs.last().filter(|_| runs.len() > 1) else {
			return Self::plan(effects, blend_mode, fill, layered);
		};

		let mut planned = Self::plan(&effects.run(last_run.clone()), blend_mode, 1., layered);
		planned.flattened += last_run.start;
		planned.trailing_masks = planned.trailing_masks.start + last_run.start..planned.trailing_masks.end + last_run.start;
		for part in planned.layers.iter_mut().flat_map(|layer| &mut layer.parts) {
			if let LayerPart::Piece(piece) = part {
				piece.capture.index += last_run.start;
			}
		}
		planned
	}

	/// The index of the layer holding the content.
	pub fn content_layer(&self) -> Option<usize> {
		self.layers.iter().position(EffectLayer::holds_content)
	}

	/// Whether the masks after every other effect fade each finished layer (see [`Self::trailing_masks`]).
	pub fn fades_layers(&self) -> bool {
		!self.trailing_masks.is_empty()
	}

	/// Whether the trailing masks fade the cover [`Self::knockout_cover`] gives the piece that `capture` names, as they do where the content's layer covers it from another layer.
	pub fn masks_fade_knockout_cover(&self, capture: EffectCapture) -> bool {
		self.fades_layers() && self.layer_of(capture).is_some_and(|layer| Some(layer) != self.content_layer())
	}

	/// How strongly the content's layer covers the layered piece beneath it that `capture` names, as a multiple of that layer's alpha, which a knockout hides the piece behind:
	/// fully from the same layer, through the item's `opacity` from another, and not at all when the content blends by another mode and mixes with the piece instead.
	/// A piece shown through `1 - cover × a` of the content's layer of alpha `a` is knocked out by `(1 - alpha) / (1 - cover × a)`, for the content's full-strength `alpha`.
	pub fn knockout_cover(&self, capture: EffectCapture, opacity: f32) -> f32 {
		let (Some(piece_layer), Some(content_layer)) = (self.layer_of(capture), self.content_layer()) else {
			return 0.;
		};

		if piece_layer == content_layer {
			1.
		} else if self.layers[content_layer].blend_mode == BlendMode::Normal {
			opacity
		} else {
			0.
		}
	}

	/// The layer drawing the piece of an effect that `capture` names, if it's a layered piece, and the piece there (its first part, for one split in two).
	pub fn piece(&self, capture: EffectCapture) -> Option<(usize, EffectPiece)> {
		self.layers.iter().enumerate().find_map(|(index, layer)| {
			layer.parts.iter().find_map(|part| match part {
				LayerPart::Piece(piece) if piece.capture == capture => Some((index, *piece)),
				_ => None,
			})
		})
	}

	/// The layer drawing the piece of an effect that `capture` names, if it's a layered piece.
	pub fn layer_of(&self, capture: EffectCapture) -> Option<usize> {
		self.piece(capture).map(|(layer, _)| layer)
	}

	/// Whether a part of an interior piece blends with what shows through the content's fill, which reads the share [`EffectCapture::THROUGH_MASK`] names.
	#[cfg(test)]
	pub fn reads_through_share(&self) -> bool {
		(self.layers.iter().flat_map(|layer| &layer.parts)).any(|part| {
			matches!(
				part,
				LayerPart::Piece(EffectPiece {
					place: PiecePlace::Through { .. } | PiecePlace::IntoContent { over: true },
					..
				})
			)
		})
	}
}

/// Whether an effect row can draw as layered pieces, rather than being carried out within the content's image.
/// Blurs, adjustments, masks, and feathers process the pixels of everything before them, and blend modes CSS lacks can't blend against the backdrop in SVG or a GPU layer,
/// though interior pieces that `blend_into_content` blend with the content alone, which the renderers do themselves.
fn can_be_layered(operation: &EffectOperation, blend_into_content: bool) -> bool {
	let layers_in = |(piece, blend_mode): (EffectPiece, BlendMode)| blend_mode.to_svg_style_name().is_some() || (blend_into_content && piece.place == PiecePlace::Within);
	!is_pixel_filter(operation) && pieces(0, operation).into_iter().all(layers_in)
}

/// Whether a mode blends a color onto what lies beneath along a straight line in it (`m × backdrop + n` for each channel), which lets a piece in it split exactly by the fill.
fn is_straight_along_backdrop(blend_mode: BlendMode) -> bool {
	matches!(blend_mode, BlendMode::Normal | BlendMode::Multiply | BlendMode::Screen | BlendMode::Exclusion | BlendMode::HardLight)
}

/// Whether an effect row processes the pixels of the content and everything drawn with it so far, as a blur, an adjustment, a mask, a feather, or another filter does.
/// A solid paint overlay decodes into the color overlay adjustment, but it paints within the content like any other paint overlay.
pub fn is_pixel_filter(operation: &EffectOperation) -> bool {
	match operation {
		// TODO: Make a mask between other effects fade each layer drawn before it too, as the trailing masks do, rather than flattening those layers.
		// TODO: Likewise for a trailing feather, faded by the footprint of every layer drawn together (not the content's alone, which would erase shadows past it).
		EffectOperation::Blur { .. } | EffectOperation::Mask { .. } | EffectOperation::Feather { .. } => true,
		operation if crate::filters::is_filter(operation) => true,
		EffectOperation::Adjustment(adjustment) => !matches!(adjustment, Adjustment::ColorOverlay { .. }),
		_ => false,
	}
}

/// How many of the `operations`, from the first, run through the last pixel filter among them, which merges the pieces before it into the content's image.
fn merged_into_content(operations: &[EffectOperation]) -> usize {
	(operations.iter())
		.rposition(|operation| !operation.changes_nothing() && is_pixel_filter(operation))
		.map_or(0, |index| index + 1)
}

/// The effects carried out in the content's own image, which make a clipping base's shape: every row through the last pixel filter, which merges the pieces before it into that image, then the trailing masks fading it.
/// The other pieces are cast from or laid over the content, so for PSD interop they don't shape a clip. Borrowed if nothing is left out.
pub fn content_image_effects(effects: &Effects) -> Cow<'_, Effects> {
	let operations: Vec<EffectOperation> = effects.operations().collect();
	let trailing_masks = trailing_masks(&operations);
	let merged = merged_into_content(&operations[..trailing_masks.start]);
	if merged == trailing_masks.start {
		return Cow::Borrowed(effects);
	}

	Cow::Owned(Effects((0..merged).chain(trailing_masks).filter_map(|index| effects.0.clone_item(index)).collect()))
}

/// The rows from the first mask after every other effect that changes something to the end, or none without such a mask or an effect before it.
fn trailing_masks(operations: &[EffectOperation]) -> Range<usize> {
	let no_rows = operations.len()..operations.len();
	let is_mask = |operation: &EffectOperation| matches!(operation, EffectOperation::Mask { .. });

	let Some(last_other) = operations.iter().rposition(|operation| !operation.changes_nothing() && !is_mask(operation)) else {
		return no_rows;
	};
	let first_mask = operations[last_other + 1..].iter().position(|operation| !operation.changes_nothing());
	first_mask.map_or(no_rows, |offset| last_other + 1 + offset..operations.len())
}

/// The pieces the effect at `index` draws, each with the mode it blends by, leaving out those painting no color.
fn pieces(index: usize, operation: &EffectOperation) -> Vec<(EffectPiece, BlendMode)> {
	// Only a drop shadow can show through the content
	let knocks_out = !matches!(operation, EffectOperation::DropShadow { knockout: false, .. });
	let piece = |plane: usize, place: PiecePlace, color: Option<Color>, blend_mode: BlendMode| {
		let capture = EffectCapture { index, plane };
		let blend_mode = piece_blend_mode(color, blend_mode);
		let piece = EffectPiece {
			capture,
			place,
			blend_mode,
			knocks_out: knocks_out && place == PiecePlace::Beneath,
		};
		color.is_none_or(|color| color.a() > 0.).then_some((piece, blend_mode))
	};
	let glow_color = |paint: &Graphic| match GlowFill::of(paint) {
		Some(GlowFill::Solid(color)) => Some(color),
		_ => None,
	};

	use PiecePlace::*;
	let pieces = match operation {
		EffectOperation::DropShadow { color, blend_mode, .. } => vec![piece(0, Beneath, Some(*color), *blend_mode)],
		EffectOperation::OuterGlow { color, blend_mode, .. } => vec![piece(0, Beneath, glow_color(color), *blend_mode)],
		EffectOperation::InnerShadow { color, blend_mode, .. } | EffectOperation::Satin { color, blend_mode, .. } => vec![piece(0, Within, Some(*color), *blend_mode)],
		EffectOperation::InnerGlow { color, blend_mode, .. } => vec![piece(0, Within, glow_color(color), *blend_mode)],
		EffectOperation::Outline {
			paint,
			blend_mode,
			weight,
			align,
			contour_gradient,
		} => {
			let color = match OutlineFill::of(paint, *contour_gradient) {
				Some(OutlineFill::Solid(color)) => Some(color),
				_ => None,
			};
			let (outside, inside) = outline_extents(*weight, *align);
			let outside = (outside > 0.).then(|| piece(0, Beneath, color, *blend_mode));
			let inside = (inside > 0.).then(|| piece(1, Within, color, *blend_mode));
			outside.into_iter().chain(inside).collect()
		}
		// Planes 0 and 1 light the slopes inside the content with the highlight and shadow, and 2 and 3 those outside it
		EffectOperation::BevelAndEmboss {
			style,
			highlight_color,
			highlight_blend_mode,
			shadow_color,
			shadow_blend_mode,
			..
		} => {
			let (highlight, shadow) = ((Some(*highlight_color), *highlight_blend_mode), (Some(*shadow_color), *shadow_blend_mode));
			let inside = (*style != BevelStyle::OuterBevel).then(|| [piece(0, Within, highlight.0, highlight.1), piece(1, Within, shadow.0, shadow.1)]);
			let outside = (*style != BevelStyle::InnerBevel).then(|| [piece(2, Beneath, highlight.0, highlight.1), piece(3, Beneath, shadow.0, shadow.1)]);
			inside.into_iter().chain(outside).flatten().collect()
		}
		EffectOperation::PaintOverlay { blend_mode, .. } => vec![piece(0, Within, None, *blend_mode)],
		EffectOperation::Adjustment(Adjustment::ColorOverlay { color, blend_mode, .. }) => vec![piece(0, Within, Some(*color), *blend_mode)],
		EffectOperation::Blur { .. }
		| EffectOperation::Mask { .. }
		| EffectOperation::Feather { .. }
		| EffectOperation::ConvolutionKernel { .. }
		| EffectOperation::Emboss { .. }
		| EffectOperation::FindEdges
		| EffectOperation::MinimumMaximum { .. }
		| EffectOperation::Sharpen { .. }
		| EffectOperation::HighPass { .. }
		| EffectOperation::AddNoise { .. }
		| EffectOperation::Dissolve { .. }
		| EffectOperation::Adjustment(_) => Vec::new(),
	};
	pieces.into_iter().flatten().collect()
}

/// The mode a piece of a solid `color` (or of no single color) blends by, which is normal where it blends the same either way, whatever lies beneath it, like black multiplying or white screening.
pub fn piece_blend_mode(color: Option<Color>, blend_mode: BlendMode) -> BlendMode {
	let channels = color.map(|color| [color.r(), color.g(), color.b()]);
	let all = |value: f32| channels.is_some_and(|channels| channels.iter().all(|&channel| channel == value));

	let normal = match blend_mode {
		BlendMode::Multiply | BlendMode::Darken => all(0.),
		BlendMode::Screen | BlendMode::Lighten => all(1.),
		_ => false,
	};
	if normal { BlendMode::Normal } else { blend_mode }
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::effects::EffectKind;

	/// The default effect of `kind` in another color and blend mode.
	fn shadow(kind: EffectKind, new_color: Color, new_blend_mode: BlendMode) -> EffectOperation {
		let mut operation = EffectOperation::default_for(kind);
		if let EffectOperation::DropShadow { color, blend_mode, .. } | EffectOperation::InnerShadow { color, blend_mode, .. } = &mut operation {
			(*color, *blend_mode) = (new_color, new_blend_mode);
		}
		operation
	}

	fn drop_shadow(color: Color, blend_mode: BlendMode) -> EffectOperation {
		shadow(EffectKind::DropShadow, color, blend_mode)
	}

	fn inner_shadow(color: Color, blend_mode: BlendMode) -> EffectOperation {
		shadow(EffectKind::InnerShadow, color, blend_mode)
	}

	fn capture(index: usize, plane: usize) -> EffectCapture {
		EffectCapture { index, plane }
	}

	fn modes(layers: &EffectLayers) -> Vec<BlendMode> {
		layers.layers.iter().map(|layer| layer.blend_mode).collect()
	}

	/// Each layer's parts as the row and plane of each piece with its place, or `None` for the content.
	fn parts(layers: &EffectLayers) -> Vec<Vec<Option<(usize, usize, PiecePlace)>>> {
		let part = |part: &LayerPart| match part {
			LayerPart::Piece(piece) => Some((piece.capture.index, piece.capture.plane, piece.place)),
			LayerPart::Content => None,
		};
		layers.layers.iter().map(|layer| layer.parts.iter().map(part).collect()).collect()
	}

	const RED: Color = Color::from_rgbaf32_unchecked(1., 0., 0., 1.);
	const INTO: PiecePlace = PiecePlace::IntoContent { over: false };

	#[test]
	fn effects_blending_normally_draw_as_one_layer_with_the_content() {
		// The default black shadow multiplies, which is the same as blending normally
		let effects = Effects::from_operations([EffectOperation::default_for(EffectKind::DropShadow), EffectOperation::default_for(EffectKind::Satin)]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 0.5, true);

		assert_eq!(layers.flattened, 0);
		assert_eq!(modes(&layers), [BlendMode::Normal]);
		assert_eq!(parts(&layers), [[Some((0, 0, PiecePlace::Beneath)), None, Some((1, 0, PiecePlace::Within))]]);

		// At a full fill, that layer draws the same with everything flattened into the content's image
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., true);
		assert_eq!(layers.flattened, 2);
		assert_eq!(parts(&layers), [[None]]);

		// White screening is the same as white drawn normally
		let screen = drop_shadow(Color::WHITE, BlendMode::Screen);
		assert_eq!(modes(&EffectLayers::plan(&Effects::from_operations([screen]), BlendMode::Normal, 1., true)), [BlendMode::Normal]);
	}

	#[test]
	fn interior_pieces_of_normal_content_blend_within_its_layer_at_a_full_fill() {
		let red = RED.with_alpha(0.75);
		let effects = Effects::from_operations([drop_shadow(red, BlendMode::Multiply), inner_shadow(red, BlendMode::Screen), inner_shadow(red, BlendMode::Normal)]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., true);
		assert_eq!(modes(&layers), [BlendMode::Multiply, BlendMode::Normal]);
		assert_eq!(parts(&layers)[1], [None, Some((1, 0, INTO)), Some((2, 0, PiecePlace::Within))]);

		// Content in a mode of its own leaves each blended piece a layer of its own
		let layers = EffectLayers::plan(&effects, BlendMode::Overlay, 1., true);
		assert_eq!(modes(&layers), [BlendMode::Multiply, BlendMode::Overlay, BlendMode::Screen, BlendMode::Normal]);

		// A mode CSS lacks is drawn normally, so it's planned that way, and an interior piece blending within the content's layer may take one too
		let layers = EffectLayers::plan(&effects, BlendMode::LinearBurn, 1., true);
		assert_eq!(modes(&layers), [BlendMode::Multiply, BlendMode::Normal]);
		let linear_burn = Effects::from_operations([drop_shadow(red, BlendMode::Multiply), inner_shadow(red, BlendMode::LinearBurn)]);
		assert_eq!(EffectLayers::plan(&linear_burn, BlendMode::Normal, 1., true).flattened, 0);
		assert_eq!(EffectLayers::plan(&linear_burn, BlendMode::Normal, 0.5, true).flattened, 2);
	}

	#[test]
	fn interior_pieces_split_by_a_partial_fill() {
		let red = RED.with_alpha(0.75);

		// A mode straight along what lies beneath leaves its part through the fill beneath the content's layer
		let screen = EffectLayers::plan(&Effects::from_operations([inner_shadow(red, BlendMode::Screen)]), BlendMode::Normal, 0.5, true);
		assert_eq!(modes(&screen), [BlendMode::Screen, BlendMode::Normal]);
		assert_eq!(parts(&screen), [vec![Some((0, 0, PiecePlace::Through { over: false }))], vec![None, Some((0, 0, INTO))]]);
		assert!(screen.reads_through_share());

		// One that bends puts it over the content's layer, which keeps the rest
		let overlay = EffectLayers::plan(&Effects::from_operations([inner_shadow(red, BlendMode::Overlay)]), BlendMode::Normal, 0.5, true);
		assert_eq!(modes(&overlay), [BlendMode::Normal, BlendMode::Overlay]);
		let over = PiecePlace::IntoContent { over: true };
		assert_eq!(parts(&overlay), [vec![None, Some((0, 0, over))], vec![Some((0, 0, PiecePlace::Through { over: true }))]]);

		// Normal pieces blend the same either way and stay whole
		let normal = EffectLayers::plan(&Effects::from_operations([inner_shadow(red, BlendMode::Normal)]), BlendMode::Normal, 0.5, true);
		assert_eq!(parts(&normal), [[None, Some((0, 0, PiecePlace::Within))]]);
		assert!(!normal.reads_through_share());
	}

	#[test]
	fn interior_pieces_after_one_over_the_content_go_over_it_too() {
		let red = RED.with_alpha(0.75);
		let (over, through_over) = (PiecePlace::IntoContent { over: true }, PiecePlace::Through { over: true });

		// A piece that bends goes over the content, and so does each piece after it, normal or straight, to keep their order
		let effects = Effects::from_operations([inner_shadow(red, BlendMode::Overlay), inner_shadow(red, BlendMode::Normal), inner_shadow(red, BlendMode::Multiply)]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 0.5, true);
		assert_eq!(modes(&layers), [BlendMode::Normal, BlendMode::Overlay, BlendMode::Normal, BlendMode::Multiply]);
		assert_eq!(
			parts(&layers),
			[
				vec![None, Some((0, 0, over)), Some((1, 0, over)), Some((2, 0, over))],
				vec![Some((0, 0, through_over))],
				vec![Some((1, 0, through_over))],
				vec![Some((2, 0, through_over))]
			]
		);

		// Pieces before it keep their places
		let effects = Effects::from_operations([inner_shadow(red, BlendMode::Multiply), inner_shadow(red, BlendMode::Overlay)]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 0.5, true);
		assert_eq!(parts(&layers)[0], [Some((0, 0, PiecePlace::Through { over: false }))]);
		assert_eq!(parts(&layers)[1], [None, Some((0, 0, INTO)), Some((1, 0, over))]);
	}

	#[test]
	fn interior_pieces_go_over_the_content_where_its_layer_covers_what_lies_beneath_it() {
		let red = RED.with_alpha(0.75);
		let through_over = PiecePlace::Through { over: true };

		let with_knockout = |knocks_out: bool| {
			let mut shadow = drop_shadow(Color::BLACK.with_alpha(0.75), BlendMode::Multiply);
			if let EffectOperation::DropShadow { knockout, .. } = &mut shadow {
				*knockout = knocks_out;
			}
			shadow
		};

		// A shadow not knocked out beneath the content in its layer would hide even a straight piece's part beneath that layer
		let layers = EffectLayers::plan(&Effects::from_operations([with_knockout(false), inner_shadow(red, BlendMode::Screen)]), BlendMode::Normal, 0.5, true);
		assert_eq!(modes(&layers), [BlendMode::Normal, BlendMode::Screen]);
		assert_eq!(parts(&layers)[1], [Some((1, 0, through_over))]);

		// A knocked-out shadow doesn't cover it there
		let layers = EffectLayers::plan(&Effects::from_operations([with_knockout(true), inner_shadow(red, BlendMode::Screen)]), BlendMode::Normal, 0.5, true);
		assert_eq!(parts(&layers)[0], [Some((1, 0, PiecePlace::Through { over: false }))]);

		// One flattened for a mode CSS lacks covers it too
		let mut flattened_shadow = with_knockout(false);
		if let EffectOperation::DropShadow { blend_mode, .. } = &mut flattened_shadow {
			*blend_mode = BlendMode::LinearBurn;
		}
		let layers = EffectLayers::plan(&Effects::from_operations([flattened_shadow, inner_shadow(red, BlendMode::Screen)]), BlendMode::Normal, 0.5, true);
		assert_eq!(layers.flattened, 1);
		assert_eq!(parts(&layers)[1], [Some((1, 0, through_over))]);
	}

	#[test]
	fn outlines_and_bevels_draw_pieces_on_both_sides_of_the_content() {
		let outline = EffectOperation::Outline {
			paint: Graphic::from(RED),
			blend_mode: BlendMode::Multiply,
			weight: 4.,
			align: vector_types::vector::style::StrokeAlign::Center,
			contour_gradient: false,
		};
		let mut bevel = EffectOperation::default_for(EffectKind::BevelAndEmboss);
		if let EffectOperation::BevelAndEmboss { style, highlight_color, .. } = &mut bevel {
			*style = BevelStyle::Emboss;
			// A highlight painting nothing draws no pieces
			*highlight_color = Color::TRANSPARENT;
		}
		let layers = EffectLayers::plan(&Effects::from_operations([outline, bevel]), BlendMode::Normal, 1., true);

		let all: Vec<_> = parts(&layers).into_iter().flatten().collect();
		let (beneath, within) = (PiecePlace::Beneath, PiecePlace::Within);
		assert_eq!(all, [Some((0, 0, beneath)), Some((1, 3, beneath)), None, Some((0, 1, INTO)), Some((1, 1, within))]);
	}

	#[test]
	fn rows_that_change_nothing_neither_flatten_nor_split_the_others() {
		let no_blur = EffectOperation::Blur {
			radius_mask: Graphic::default(),
			radius: 0.,
			box_blur: false,
			gamma: false,
		};
		let layers = EffectLayers::plan(&Effects::from_operations([drop_shadow(RED, BlendMode::Multiply), no_blur]), BlendMode::Normal, 1., true);
		assert_eq!(layers.flattened, 0);
		assert_eq!(modes(&layers), [BlendMode::Multiply, BlendMode::Normal]);

		// A solid overlay at no opacity paints nothing, so the normal pieces around it stay one layer with the content
		let no_overlay = EffectOperation::Adjustment(Adjustment::ColorOverlay {
			color: RED,
			blend_mode: BlendMode::LinearBurn,
			opacity: 0.,
		});
		let effects = Effects::from_operations([drop_shadow(RED, BlendMode::Normal), no_overlay, inner_shadow(RED, BlendMode::Normal)]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 0.5, true);
		assert_eq!(layers.flattened, 0);
		assert_eq!(parts(&layers), [[Some((0, 0, PiecePlace::Beneath)), None, Some((2, 0, PiecePlace::Within))]]);
	}

	fn mask() -> EffectOperation {
		EffectOperation::default_for(EffectKind::Mask)
	}

	#[test]
	fn masks_between_other_effects_flatten_those_before_them_into_the_content() {
		// The shadow before the mask is hidden with the content, while the one after it is cast from the masked content as a layer of its own
		let effects = Effects::from_operations([drop_shadow(RED, BlendMode::Multiply), mask(), drop_shadow(RED, BlendMode::Screen)]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., true);
		assert_eq!(layers.flattened, 2);
		assert_eq!(modes(&layers), [BlendMode::Screen, BlendMode::Normal]);
		assert!(!layers.fades_layers());

		// Masks with no other effect before them shape the content alone
		let layers = EffectLayers::plan(&Effects::from_operations([mask(), mask()]), BlendMode::Normal, 0.5, true);
		assert_eq!((layers.flattened, layers.fades_layers()), (2, false));
	}

	#[test]
	fn trailing_masks_fade_each_layer_in_its_own_blend_mode() {
		let no_blur = EffectOperation::Blur {
			radius_mask: Graphic::default(),
			radius: 0.,
			box_blur: false,
			gamma: false,
		};

		// The shadow before the masks keeps its layer and its mode, and the content's layer covers it as far as the masks leave of that layer
		let effects = Effects::from_operations([drop_shadow(RED, BlendMode::Multiply), mask()]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., true);
		assert_eq!((layers.flattened, layers.trailing_masks.clone()), (0, 1..2));
		assert_eq!(modes(&layers), [BlendMode::Multiply, BlendMode::Normal]);
		assert!(layers.masks_fade_knockout_cover(capture(0, 0)));

		// Rows that change nothing don't end the trailing masks, and a mask before the other effects still shapes the content
		let effects = Effects::from_operations([mask(), drop_shadow(RED, BlendMode::Multiply), mask(), no_blur, mask()]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., true);
		assert_eq!((layers.flattened, layers.trailing_masks.clone()), (1, 2..5));
		assert_eq!(modes(&layers), [BlendMode::Multiply, BlendMode::Normal]);

		// A piece in the content's own layer fades with it as a whole
		let normal = Effects::from_operations([drop_shadow(RED, BlendMode::Normal), mask()]);
		let layers = EffectLayers::plan(&normal, BlendMode::Normal, 0.5, true);
		assert_eq!((layers.flattened, layers.trailing_masks.clone()), (0, 1..2));
		assert!(!layers.masks_fade_knockout_cover(capture(0, 0)));

		// With everything flattened, the masks are carried out within the content's image like any other
		let layers = EffectLayers::plan(&normal, BlendMode::Normal, 1., true);
		assert_eq!((layers.flattened, layers.trailing_masks.clone()), (2, 2..2));
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., false);
		assert_eq!((layers.flattened, layers.trailing_masks.clone()), (5, 5..5));
		let unmerged = Effects::from_operations([drop_shadow(RED, BlendMode::LinearBurn), drop_shadow(RED, BlendMode::Multiply), mask()]);
		let layers = EffectLayers::plan(&unmerged, BlendMode::Normal, 1., true);
		assert_eq!((layers.flattened, layers.trailing_masks.clone()), (3, 3..3));
	}

	#[test]
	fn effects_before_a_blur_or_a_mode_css_lacks_are_flattened() {
		let blur = EffectOperation::Blur {
			radius_mask: Graphic::default(),
			radius: 2.,
			box_blur: false,
			gamma: false,
		};

		let effects = Effects::from_operations([drop_shadow(RED, BlendMode::Multiply), blur, drop_shadow(RED, BlendMode::Multiply)]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., true);
		assert_eq!(layers.flattened, 2);
		assert_eq!(modes(&layers), [BlendMode::Multiply, BlendMode::Normal]);

		let effects = Effects::from_operations([inner_shadow(RED, BlendMode::Screen), drop_shadow(RED, BlendMode::LinearBurn), inner_shadow(RED, BlendMode::Multiply)]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 0.5, true);
		assert_eq!(layers.flattened, 2);

		// A flattened shadow left unmerged by a blur would cover the layered shadow after it, so everything flattens
		let effects = Effects::from_operations([drop_shadow(RED, BlendMode::LinearBurn), drop_shadow(RED, BlendMode::Multiply)]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., true);
		assert_eq!(layers.flattened, 2);
		assert_eq!(modes(&layers), [BlendMode::Normal]);

		// Clipped content flattens everything
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., false);
		assert_eq!(layers.flattened, 2);
		assert_eq!(parts(&layers), [[None]]);
	}

	#[test]
	fn content_images_keep_the_filters_and_what_they_merge_but_not_the_pieces_around_them() {
		let blur = EffectOperation::Blur {
			radius_mask: Graphic::default(),
			radius: 2.,
			box_blur: false,
			gamma: false,
		};
		let shadow = || drop_shadow(RED, BlendMode::Multiply);
		let kinds = |operations: Vec<EffectOperation>| -> Vec<EffectKind> { content_image_effects(&Effects::from_operations(operations)).operations().map(|operation| operation.kind()).collect() };

		// A shadow is cast from the content, unless a later filter merges it into the content's image
		assert!(kinds(vec![shadow()]).is_empty());
		assert_eq!(kinds(vec![blur.clone(), shadow()]), [EffectKind::Blur]);
		let merged = Effects::from_operations([shadow(), blur.clone()]);
		assert!(matches!(content_image_effects(&merged), Cow::Borrowed(_)));

		// Trailing masks fade the content's image as they fade each layer
		assert_eq!(kinds(vec![shadow(), mask()]), [EffectKind::Mask]);
		let filtered = vec![mask(), inner_shadow(RED, BlendMode::Screen), blur, shadow(), mask()];
		assert_eq!(kinds(filtered), [EffectKind::Mask, EffectKind::InnerShadow, EffectKind::Blur, EffectKind::Mask]);
	}

	#[test]
	fn knocked_out_pieces_hide_behind_however_the_content_covers_them() {
		let same_layer = EffectLayers::plan(&Effects::from_operations([drop_shadow(RED, BlendMode::Normal)]), BlendMode::Normal, 0.5, true);
		assert_eq!(same_layer.knockout_cover(capture(0, 0), 0.4), 1.);

		let own_layer = EffectLayers::plan(&Effects::from_operations([drop_shadow(RED, BlendMode::Multiply)]), BlendMode::Normal, 0.5, true);
		assert_eq!(own_layer.knockout_cover(capture(0, 0), 0.4), 0.4);

		let blended_content = EffectLayers::plan(&Effects::from_operations([drop_shadow(RED, BlendMode::Multiply)]), BlendMode::Screen, 0.5, true);
		assert_eq!(blended_content.knockout_cover(capture(0, 0), 0.4), 0.);

		// A shadow with knockout off shows through the content
		let mut shown_through = drop_shadow(RED, BlendMode::Normal);
		if let EffectOperation::DropShadow { knockout, .. } = &mut shown_through {
			*knockout = false;
		}
		let layers = EffectLayers::plan(&Effects::from_operations([shown_through]), BlendMode::Normal, 0.5, true);
		assert!(layers.piece(capture(0, 0)).is_some_and(|(_, piece)| !piece.knocks_out));
	}
}
