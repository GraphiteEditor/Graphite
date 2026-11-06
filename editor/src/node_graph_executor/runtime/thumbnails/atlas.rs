use super::{ThumbnailContent, cell_size};
use glam::{DAffine2, UVec2};
use graph_craft::document::NodeId;
use graphene_std::renderer::vello_scene_builder::VelloSceneBuilder;
use graphene_std::renderer::{RenderOutputType, RenderParams};
use graphene_std::transform::Footprint;
use std::collections::HashMap;
use vello_gpu::kurbo::{Affine, Rect};
use vello_gpu::peniko::Fill;

/// The most cells an atlas can hold within the maximum texture size, since its rows are at most as many as its columns.
pub fn max_cells_per_atlas(cell_size: UVec2, max_texture_size: u32) -> usize {
	let columns = (max_texture_size / cell_size.max_element()).max(1) as usize;
	columns * columns
}

/// The thumbnails of the layers in view, in equally sized cells filling the atlas row by row.
pub struct ThumbnailAtlasLayout {
	pub cell_size: UVec2,
	columns: u32,
	pub layers: Vec<NodeId>,
}

impl ThumbnailAtlasLayout {
	pub fn new(layers: Vec<NodeId>, scale: f64) -> Self {
		let columns = (layers.len() as f64).sqrt().ceil().max(1.) as u32;

		Self {
			cell_size: cell_size(scale),
			columns,
			layers,
		}
	}

	pub fn size(&self) -> UVec2 {
		let rows = (self.layers.len() as u32).div_ceil(self.columns).max(1);
		UVec2::new(self.columns, rows) * self.cell_size
	}

	pub fn cell_origin(&self, index: usize) -> UVec2 {
		let index = index as u32;
		UVec2::new(index % self.columns, index / self.columns) * self.cell_size
	}

	pub fn cell_pixels(&self, atlas_pixels: &[u8], index: usize) -> Vec<u8> {
		let atlas_row_length = self.size().x as usize * 4;
		let row_length = self.cell_size.x as usize * 4;
		let origin = self.cell_origin(index).as_usizevec2();

		(origin.y..origin.y + self.cell_size.y as usize)
			.filter_map(|row| {
				let start = row * atlas_row_length + origin.x * 4;
				atlas_pixels.get(start..start + row_length)
			})
			.flatten()
			.copied()
			.collect()
	}
}

/// Draws the content of each layer in the layout centered within its cell.
pub fn draw_atlas(scene: &mut VelloSceneBuilder, layout: &ThumbnailAtlasLayout, contents: &HashMap<NodeId, ThumbnailContent>) {
	let render_params = RenderParams {
		footprint: Footprint::BOUNDLESS,
		render_output_type: RenderOutputType::Vello,
		thumbnail: true,
		..Default::default()
	};
	let cell_size = layout.cell_size.as_dvec2();

	for (index, layer) in layout.layers.iter().enumerate() {
		let Some(content) = contents.get(layer) else { continue };

		let origin = layout.cell_origin(index).as_dvec2();
		let [min, max] = content.bounds();
		let transform = DAffine2::from_translation(origin) * DAffine2::from_scale(cell_size / (max - min)) * DAffine2::from_translation(-min);

		// The clip also limits content filling the whole scene, like a background, to the cell
		let cell = Rect::new(origin.x, origin.y, origin.x + cell_size.x, origin.y + cell_size.y);
		scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &cell);
		content.graphic().render_to_vello(scene, transform, &render_params);
		scene.pop_layer();
	}
}

/// The RGBA pixels with runs of fully transparent ones left out, which most of a thumbnail's margins are.
/// Encoded as pairs of little-endian `u32` counts of skipped and kept pixels, each pair followed by the kept pixels' bytes.
#[cfg(not(all(target_family = "wasm", feature = "wasm")))]
pub fn encode_transparent_runs(pixels: &[u8]) -> Vec<u8> {
	let (pixels, _) = pixels.as_chunks::<4>();
	let mut encoded = Vec::new();

	let mut index = 0;
	while index < pixels.len() {
		let skipped = pixels[index..].iter().take_while(|pixel| pixel[3] == 0).count();
		let kept_start = index + skipped;
		let kept = pixels[kept_start..].iter().take_while(|pixel| pixel[3] != 0).count();

		encoded.extend_from_slice(&(skipped as u32).to_le_bytes());
		encoded.extend_from_slice(&(kept as u32).to_le_bytes());
		for pixel in &pixels[kept_start..kept_start + kept] {
			encoded.extend_from_slice(pixel);
		}

		index = kept_start + kept;
	}

	encoded
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn a_cell_is_copied_out_of_the_atlas_row_by_row() {
		let layout = ThumbnailAtlasLayout {
			cell_size: UVec2::new(1, 2),
			columns: 2,
			layers: vec![NodeId(1), NodeId(2)],
		};
		// Each pixel's red channel numbers it in reading order
		let atlas_pixels = (0..4).flat_map(|pixel| [pixel, 0, 0, 255]).collect::<Vec<u8>>();

		assert_eq!(layout.cell_pixels(&atlas_pixels, 1), [[1, 0, 0, 255], [3, 0, 0, 255]].concat());
	}

	#[test]
	fn transparent_runs_are_left_out_and_opaque_pixels_kept() {
		let transparent = [9, 9, 9, 0];
		let red = [255, 0, 0, 255];
		let pixels = [transparent, transparent, red, red, transparent, red].concat();

		let pair = |skipped: u32, kept: u32| [skipped.to_le_bytes(), kept.to_le_bytes()].concat();
		let expected = [pair(2, 2), red.to_vec(), red.to_vec(), pair(1, 1), red.to_vec()].concat();
		assert_eq!(encode_transparent_runs(&pixels), expected);
	}
}
