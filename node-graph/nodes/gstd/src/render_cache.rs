//! Tile-based render caching for efficient viewport panning.

use core_types::math::bbox::AxisAlignedBbox;
use core_types::transform::{Footprint, RenderQuality, Transform};
use core_types::{CloneVarArgs, Context, Ctx, ExtractAll, ExtractAnimationTime, ExtractPointerPosition, ExtractRealTime, OwnedContextImpl};
use glam::{DAffine2, DVec2, IVec2, UVec2};
use graph_craft::document::value::RenderOutput;
use graph_craft::wasm_application_io::WasmEditorApi;
use graphene_application_io::{ApplicationIo, ImageTexture};
use rendering::{RenderOutputType as RenderOutputTypeRequest, RenderParams};
use std::collections::HashSet;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use crate::render_node::RenderOutputType;

pub const TILE_SIZE: u32 = 256;
pub const MAX_CACHE_MEMORY_BYTES: usize = 512 * 1024 * 1024;
const BYTES_PER_PIXEL: usize = 4;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct TileCoord {
	pub x: i32,
	pub y: i32,
}

#[derive(Debug, Clone)]
pub struct CachedRegion {
	pub texture: wgpu::Texture,
	pub texture_size: UVec2,
	pub tiles: Vec<TileCoord>,
	pub metadata: rendering::RenderMetadata,
	last_access: u64,
	memory_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CacheKey {
	pub max_region_area: u32,
	pub render_mode_hash: u64,
	pub device_scale: u64,
	pub zoom: u64,
	pub rotation: u64,
	pub hide_artboards: bool,
	pub for_export: bool,
	pub for_mask: bool,
	pub thumbnail: bool,
	pub aligned_strokes: bool,
	pub override_paint_order: bool,
	pub animation_time_ms: i64,
	pub real_time_ms: i64,
	pub pointer: [u8; 16],
}

impl CacheKey {
	#[expect(clippy::too_many_arguments)]
	fn new(
		max_region_area: u32,
		render_mode_hash: u64,
		device_scale: f64,
		zoom: f64,
		rotation: f64,
		hide_artboards: bool,
		for_export: bool,
		for_mask: bool,
		thumbnail: bool,
		aligned_strokes: bool,
		override_paint_order: bool,
		animation_time: f64,
		real_time: f64,
		pointer: Option<DVec2>,
	) -> Self {
		let pointer_bytes = pointer
			.map(|p| {
				let mut bytes = [0u8; 16];
				bytes[..8].copy_from_slice(&p.x.to_le_bytes());
				bytes[8..].copy_from_slice(&p.y.to_le_bytes());
				bytes
			})
			.unwrap_or([0u8; 16]);
		const ROTATION_QUANTIZATION_DIGITS: i32 = 5;
		let quantization_amount = 10f64.powi(ROTATION_QUANTIZATION_DIGITS);
		let quantized_rotation = (rotation * quantization_amount).round() * quantization_amount.recip();
		Self {
			max_region_area,
			render_mode_hash,
			device_scale: device_scale.to_bits(),
			zoom: zoom.to_bits(),
			rotation: quantized_rotation.to_bits(),
			hide_artboards,
			for_export,
			for_mask,
			thumbnail,
			aligned_strokes,
			override_paint_order,
			animation_time_ms: (animation_time * 1000.0).round() as i64,
			real_time_ms: (real_time * 1000.0).round() as i64,
			pointer: pointer_bytes,
		}
	}
}

#[derive(Default, Debug)]
struct TileCacheImpl {
	regions: Vec<CachedRegion>,
	timestamp: u64,
	total_memory: usize,
	cache_key: CacheKey,
}

#[derive(Clone, Default, dyn_any::DynAny, Debug)]
pub struct TileCache(Arc<Mutex<TileCacheImpl>>);

#[derive(Debug, Clone)]
pub struct RenderRegion {
	pub tiles: Vec<TileCoord>,
}

#[derive(Debug)]
pub struct CacheQuery {
	pub cached_regions: Vec<CachedRegion>,
	pub missing_regions: Vec<RenderRegion>,
}

fn bounds_to_tiles(bounds: &AxisAlignedBbox) -> Vec<TileCoord> {
	let tile_size = TILE_SIZE as f64;
	let tile_start_x = (bounds.start.x / tile_size).floor() as i32;
	let tile_start_y = (bounds.start.y / tile_size).floor() as i32;
	let tile_end_x = (bounds.end.x / tile_size).ceil() as i32;
	let tile_end_y = (bounds.end.y / tile_size).ceil() as i32;

	let mut tiles = Vec::new();
	for y in tile_start_y..tile_end_y {
		for x in tile_start_x..tile_end_x {
			tiles.push(TileCoord { x, y });
		}
	}
	tiles
}

fn tile_bounds(coord: &TileCoord) -> AxisAlignedBbox {
	let tile_size = TILE_SIZE as f64;
	let start = DVec2::new(coord.x as f64, coord.y as f64) * tile_size;
	AxisAlignedBbox {
		start,
		end: start + DVec2::splat(tile_size),
	}
}

fn tiles_bounds(tiles: &[TileCoord]) -> AxisAlignedBbox {
	if tiles.is_empty() {
		return AxisAlignedBbox::ZERO;
	}
	let mut result = tile_bounds(&tiles[0]);
	for tile in &tiles[1..] {
		result = result.union(&tile_bounds(tile));
	}
	result
}

impl TileCacheImpl {
	fn query(&mut self, viewport_bounds: &AxisAlignedBbox, cache_key: &CacheKey, max_region_area: u32) -> CacheQuery {
		if &self.cache_key != cache_key {
			self.invalidate_all();
			self.cache_key = cache_key.clone();
		}

		let required_tiles = bounds_to_tiles(viewport_bounds);
		let required_tile_set: HashSet<_> = required_tiles.iter().cloned().collect();
		let mut cached_regions = Vec::new();
		let mut covered_tiles = HashSet::new();

		for region in &mut self.regions {
			let region_tiles: HashSet<_> = region.tiles.iter().cloned().collect();
			if region_tiles.iter().any(|t| required_tile_set.contains(t)) {
				region.last_access = self.timestamp;
				self.timestamp += 1;
				cached_regions.push(region.clone());
				covered_tiles.extend(region_tiles);
			}
		}

		let missing_tiles: Vec<_> = required_tiles.into_iter().filter(|t| !covered_tiles.contains(t)).collect();
		let missing_regions = group_into_regions(&missing_tiles, max_region_area);
		CacheQuery { cached_regions, missing_regions }
	}

	fn store_regions(&mut self, new_regions: Vec<CachedRegion>) {
		for mut region in new_regions {
			region.last_access = self.timestamp;
			self.timestamp += 1;
			self.total_memory += region.memory_size;
			self.regions.push(region);
		}
		self.evict_until_under_budget();
	}

	fn evict_until_under_budget(&mut self) {
		while self.total_memory > MAX_CACHE_MEMORY_BYTES && !self.regions.is_empty() {
			if let Some((oldest_idx, _)) = self.regions.iter().enumerate().min_by_key(|(_, r)| r.last_access) {
				let removed = self.regions.remove(oldest_idx);
				removed.texture.destroy();
				self.total_memory = self.total_memory.saturating_sub(removed.memory_size);
			} else {
				break;
			}
		}
	}

	fn invalidate_all(&mut self) {
		for region in &self.regions {
			region.texture.destroy();
		}
		self.regions.clear();
		self.total_memory = 0;
	}
}

impl TileCache {
	pub fn query(&self, viewport_bounds: &AxisAlignedBbox, cache_key: &CacheKey, max_region_area: u32) -> CacheQuery {
		self.0.lock().unwrap().query(viewport_bounds, cache_key, max_region_area)
	}

	pub fn store_regions(&self, regions: Vec<CachedRegion>) {
		self.0.lock().unwrap().store_regions(regions);
	}
}

fn group_into_regions(tiles: &[TileCoord], max_region_area: u32) -> Vec<RenderRegion> {
	if tiles.is_empty() {
		return Vec::new();
	}

	let tile_set: HashSet<_> = tiles.iter().cloned().collect();
	let mut visited = HashSet::new();
	let mut regions = Vec::new();

	for &tile in tiles {
		if visited.contains(&tile) {
			continue;
		}
		let region_tiles = flood_fill(&tile, &tile_set, &mut visited);
		let region = RenderRegion { tiles: region_tiles };
		regions.extend(split_oversized_region(region, max_region_area));
	}
	regions
}

/// Recursively subdivides a region until all sub-regions have area <= max_region_area.
/// Uses axis-aligned splits on the longest dimension.
fn split_oversized_region(region: RenderRegion, max_region_area: u32) -> Vec<RenderRegion> {
	let pixel_size = tiles_bounds(&region.tiles).size();
	let area = (pixel_size.x * pixel_size.y) as u32;

	// Base case: region is small enough
	if area <= max_region_area {
		return vec![region];
	}

	// Determine split axis: choose the longer dimension
	let split_horizontally = pixel_size.x > pixel_size.y;

	// Split tiles into two groups based on midpoint
	let mut group1 = Vec::new();
	let mut group2 = Vec::new();

	if split_horizontally {
		let min_x = region.tiles.iter().map(|t| t.x).min().unwrap();
		let max_x = region.tiles.iter().map(|t| t.x).max().unwrap();
		let mid_x = min_x + (max_x - min_x) / 2;

		for &tile in &region.tiles {
			if tile.x <= mid_x {
				group1.push(tile);
			} else {
				group2.push(tile);
			}
		}
	} else {
		let min_y = region.tiles.iter().map(|t| t.y).min().unwrap();
		let max_y = region.tiles.iter().map(|t| t.y).max().unwrap();
		let mid_y = min_y + (max_y - min_y) / 2;

		for &tile in &region.tiles {
			if tile.y <= mid_y {
				group1.push(tile);
			} else {
				group2.push(tile);
			}
		}
	}

	if group1.is_empty() || group2.is_empty() {
		log::error!("Failed to split oversized region.");
		return vec![region];
	}

	let mut result = Vec::new();
	for tiles in [group1, group2] {
		if !tiles.is_empty() {
			let sub_region = RenderRegion { tiles };
			result.extend(split_oversized_region(sub_region, max_region_area));
		}
	}

	result
}

fn flood_fill(start: &TileCoord, tile_set: &HashSet<TileCoord>, visited: &mut HashSet<TileCoord>) -> Vec<TileCoord> {
	let mut result = Vec::new();
	let mut stack = vec![*start];

	while let Some(current) = stack.pop() {
		if visited.contains(&current) || !tile_set.contains(&current) {
			continue;
		}
		visited.insert(current);
		result.push(current);

		for neighbor in [
			TileCoord { x: current.x - 1, y: current.y },
			TileCoord { x: current.x + 1, y: current.y },
			TileCoord { x: current.x, y: current.y - 1 },
			TileCoord { x: current.x, y: current.y + 1 },
		] {
			if tile_set.contains(&neighbor) && !visited.contains(&neighbor) {
				stack.push(neighbor);
			}
		}
	}
	result
}

#[node_macro::node(category(""))]
pub async fn render_output_cache<'a: 'n>(
	ctx: impl Ctx + ExtractAll + CloneVarArgs + ExtractRealTime + ExtractAnimationTime + ExtractPointerPosition + Sync,
	editor_api: &'a WasmEditorApi,
	data: impl Node<Context<'static>, Output = RenderOutput> + Send + Sync,
	#[data] tile_cache: TileCache,
) -> RenderOutput {
	let footprint = ctx.footprint();
	let Some(render_params) = ctx.vararg(0).ok().and_then(|v| v.downcast_ref::<RenderParams>()) else {
		log::warn!("render_output_cache: missing or invalid render params, falling back to direct render");
		let context = OwnedContextImpl::empty().with_footprint(*footprint);
		return data.eval(context.into_context()).await;
	};

	// Fall back to direct render for non-Vello or zero-size viewports
	let physical_resolution = footprint.resolution;
	if !matches!(render_params.render_output_type, RenderOutputTypeRequest::Vello) || physical_resolution.x == 0 || physical_resolution.y == 0 {
		let context = OwnedContextImpl::empty().with_footprint(*footprint).with_vararg(Box::new(render_params.clone()));
		return data.eval(context.into_context()).await;
	}

	let device_scale = render_params.scale;
	let zoom = footprint.decompose_scale().x;
	let rotation = footprint.decompose_rotation();

	let viewport_origin_offset = footprint.transform.translation;
	let device_origin_offset = viewport_origin_offset * device_scale;
	let viewport_bounds_device = AxisAlignedBbox {
		start: -device_origin_offset,
		end: footprint.resolution.as_dvec2() - device_origin_offset,
	};

	let max_region_area = editor_api.editor_preferences.max_render_region_area();

	let cache_key = CacheKey::new(
		max_region_area,
		render_params.render_mode as u64,
		device_scale,
		zoom,
		rotation,
		render_params.hide_artboards,
		render_params.for_export,
		render_params.for_mask,
		render_params.thumbnail,
		render_params.aligned_strokes,
		render_params.override_paint_order,
		ctx.try_animation_time().unwrap_or(0.0),
		ctx.try_real_time().unwrap_or(0.0),
		ctx.try_pointer_position(),
	);

	let cache_query = tile_cache.query(&viewport_bounds_device, &cache_key, max_region_area);

	let mut new_regions = Vec::new();
	for missing_region in &cache_query.missing_regions {
		if missing_region.tiles.is_empty() {
			continue;
		}
		let region = render_missing_region(
			missing_region,
			|ctx| data.eval(ctx),
			ctx.clone(),
			render_params,
			&footprint.transform,
			&viewport_origin_offset,
			device_scale,
		)
		.await;
		new_regions.push(region);
	}

	tile_cache.store_regions(new_regions.clone());

	let all_regions: Vec<_> = cache_query.cached_regions.into_iter().chain(new_regions.into_iter()).collect();

	// If no regions, fall back to direct render
	if all_regions.is_empty() {
		let context = OwnedContextImpl::empty().with_footprint(*footprint).with_vararg(Box::new(render_params.clone()));
		return data.eval(context.into_context()).await;
	}

	let exec = editor_api.application_io.as_ref().unwrap().gpu_executor().unwrap();
	let (output_texture, combined_metadata) = composite_cached_regions(&all_regions, physical_resolution, &device_origin_offset, &footprint.transform, exec);

	RenderOutput {
		data: RenderOutputType::Texture(ImageTexture { texture: output_texture }),
		metadata: combined_metadata,
	}
}

async fn render_missing_region<F, Fut>(
	region: &RenderRegion,
	render_fn: F,
	ctx: impl Ctx + ExtractAll + CloneVarArgs,
	render_params: &RenderParams,
	viewport_transform: &DAffine2,
	viewport_origin_offset: &DVec2,
	device_scale: f64,
) -> CachedRegion
where
	F: Fn(Context<'static>) -> Fut,
	Fut: std::future::Future<Output = RenderOutput>,
{
	let min_tile = region.tiles.iter().fold(IVec2::new(i32::MAX, i32::MAX), |acc, t| acc.min(IVec2::new(t.x, t.y)));
	let max_tile = region.tiles.iter().fold(IVec2::new(i32::MIN, i32::MIN), |acc, t| acc.max(IVec2::new(t.x, t.y)));

	let tile_count = (max_tile - min_tile) + IVec2::ONE;
	let region_pixel_size = (tile_count * TILE_SIZE as i32).as_uvec2();

	let tile_global_offset = min_tile.as_dvec2() * (TILE_SIZE as f64 / device_scale) + *viewport_origin_offset;
	let region_transform = DAffine2::from_translation(-tile_global_offset) * *viewport_transform;
	let region_footprint = Footprint {
		transform: region_transform,
		resolution: region_pixel_size,
		quality: RenderQuality::Full,
	};

	let region_params = render_params.clone();
	let region_ctx = OwnedContextImpl::from(ctx).with_footprint(region_footprint).with_vararg(Box::new(region_params)).into_context();
	let mut result = render_fn(region_ctx).await;

	let RenderOutputType::Texture(rendered_texture) = result.data else {
		unreachable!("render_missing_region: expected texture output from Vello render");
	};

	let pixel_to_document = region_transform.inverse();
	result.metadata.apply_transform(pixel_to_document);

	let memory_size = (region_pixel_size.x * region_pixel_size.y) as usize * BYTES_PER_PIXEL;

	CachedRegion {
		texture: rendered_texture.texture,
		texture_size: region_pixel_size,
		tiles: region.tiles.clone(),
		metadata: result.metadata,
		last_access: 0,
		memory_size,
	}
}

fn composite_cached_regions(
	regions: &[CachedRegion],
	output_resolution: UVec2,
	device_origin_offset: &DVec2,
	viewport_transform: &DAffine2,
	exec: &wgpu_executor::WgpuExecutor,
) -> (wgpu::Texture, rendering::RenderMetadata) {
	let device = &exec.context.device;
	let queue = &exec.context.queue;

	// TODO: Use texture pool to reuse existing unused textures instead of allocating fresh ones every time
	let output_texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("viewport_output"),
		size: wgpu::Extent3d {
			width: output_resolution.x,
			height: output_resolution.y,
			depth_or_array_layers: 1,
		},
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba8Unorm,
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
		view_formats: &[],
	});

	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("composite") });
	let mut combined_metadata = rendering::RenderMetadata::default();

	for region in regions {
		let min_tile = region.tiles.iter().fold(IVec2::new(i32::MAX, i32::MAX), |acc, t| acc.min(IVec2::new(t.x, t.y)));

		// Convert global tile position to physical pixel offset in the output texture
		let offset_pixels = min_tile * TILE_SIZE as i32 + device_origin_offset.round().as_ivec2();

		let (src_x, dst_x, width) = if offset_pixels.x >= 0 {
			(0, offset_pixels.x as u32, region.texture_size.x.min(output_resolution.x.saturating_sub(offset_pixels.x as u32)))
		} else {
			let skip = (-offset_pixels.x) as u32;
			(skip, 0, region.texture_size.x.saturating_sub(skip).min(output_resolution.x))
		};

		let (src_y, dst_y, height) = if offset_pixels.y >= 0 {
			(0, offset_pixels.y as u32, region.texture_size.y.min(output_resolution.y.saturating_sub(offset_pixels.y as u32)))
		} else {
			let skip = (-offset_pixels.y) as u32;
			(skip, 0, region.texture_size.y.saturating_sub(skip).min(output_resolution.y))
		};

		if width > 0 && height > 0 {
			encoder.copy_texture_to_texture(
				wgpu::TexelCopyTextureInfo {
					texture: &region.texture,
					mip_level: 0,
					origin: wgpu::Origin3d { x: src_x, y: src_y, z: 0 },
					aspect: wgpu::TextureAspect::All,
				},
				wgpu::TexelCopyTextureInfo {
					texture: &output_texture,
					mip_level: 0,
					origin: wgpu::Origin3d { x: dst_x, y: dst_y, z: 0 },
					aspect: wgpu::TextureAspect::All,
				},
				wgpu::Extent3d {
					width,
					height,
					depth_or_array_layers: 1,
				},
			);
		}

		let mut region_metadata = region.metadata.clone();
		region_metadata.apply_transform(*viewport_transform);
		combined_metadata.merge(&region_metadata);
	}

	queue.submit([encoder.finish()]);
	(output_texture, combined_metadata)
}
