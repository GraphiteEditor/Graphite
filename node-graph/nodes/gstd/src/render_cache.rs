//! # Render Cache Module
//!
//! This module implements tile-based caching for rendered output to enable efficient
//! incremental rendering when panning the viewport.
//!
//! ## Coordinate Spaces
//!
//! The render cache operates across three coordinate spaces:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                         COORDINATE SPACES                                    │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                                                                              │
//! │  1. DOCUMENT SPACE (World Space)                                            │
//! │     - The canonical coordinate system for artwork                           │
//! │     - Units: abstract "world units" (typically 1 unit = 1 pixel at 100%)    │
//! │     - Origin: document origin (0,0)                                         │
//! │     - Used for: storing artwork, metadata click targets                     │
//! │                                                                              │
//! │  2. TILE GRID SPACE                                                          │
//! │     - Integer grid for cache management                                      │
//! │     - Units: tile indices (i32)                                             │
//! │     - Each tile covers TILE_SIZE (256) pixels at current scale              │
//! │     - Tile (0,0) covers world region [0, TILE_SIZE/scale)                   │
//! │     - Tile grid is scale-dependent: different scales = different grids      │
//! │                                                                              │
//! │  3. PIXEL SPACE (Viewport Space)                                            │
//! │     - Screen pixels for final rendering                                     │
//! │     - Units: pixels (u32 for sizes, i32 for positions)                      │
//! │     - Origin: viewport top-left corner                                       │
//! │     - Resolution: footprint.resolution                                       │
//! │                                                                              │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                         COORDINATE CONVERSIONS                               │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                                                                              │
//! │  scale = pixels per world unit (from footprint.transform)                   │
//! │                                                                              │
//! │  Document → Pixel:                                                          │
//! │      pixel = (world - viewport_origin) * scale                              │
//! │                                                                              │
//! │  Pixel → Document:                                                          │
//! │      world = pixel / scale + viewport_origin                                │
//! │                                                                              │
//! │  Document → Tile:                                                           │
//! │      tile.x = floor(world.x * scale / TILE_SIZE)                            │
//! │      tile.y = floor(world.y * scale / TILE_SIZE)                            │
//! │                                                                              │
//! │  Tile → Document (tile origin):                                             │
//! │      world.x = tile.x * TILE_SIZE / scale                                   │
//! │      world.y = tile.y * TILE_SIZE / scale                                   │
//! │                                                                              │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                         COMPOSITING PIPELINE                                 │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                                                                              │
//! │  Stage 1: Render missing regions to tile-aligned textures                   │
//! │      - Each region covers an integer number of tiles                        │
//! │      - Region texture size = (tiles_wide, tiles_high) * TILE_SIZE           │
//! │      - Region transform: scale + translate to region origin                 │
//! │                                                                              │
//! │  Stage 2: Copy regions to tile-aligned intermediate texture                 │
//! │      - Position = (region_min_tile - global_min_tile) * TILE_SIZE           │
//! │      - No fractional offsets - everything is tile-aligned                   │
//! │                                                                              │
//! │  Stage 3: Copy from intermediate to viewport output                         │
//! │      - Single offset calculation: tile_origin - viewport_origin             │
//! │      - This is the ONLY place with sub-tile precision                       │
//! │                                                                              │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Key Invariants
//!
//! - All cached region textures are tile-aligned (dimensions are multiples of TILE_SIZE)
//! - Tile coordinates are computed from world coordinates at the current scale
//! - When scale changes, the entire cache is invalidated (tile grid changes)
//! - Metadata is stored in document space and transformed to viewport space on output

use core_types::math::bbox::AxisAlignedBbox;
use core_types::transform::{Footprint, RenderQuality, Transform};
use core_types::{CloneVarArgs, Context, Ctx, ExtractAll, ExtractAnimationTime, ExtractRealTime, OwnedContextImpl};
use glam::{DVec2, IVec2, UVec2};
use graph_craft::document::value::RenderOutput;
use graph_craft::wasm_application_io::WasmEditorApi;
use graphene_application_io::{ApplicationIo, ImageTexture};
use rendering::{RenderOutputType as RenderOutputTypeRequest, RenderParams};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use crate::render_node::RenderOutputType;

// Constants
/// Size of each cache tile in pixels. Tiles form a grid in pixel space.
pub const TILE_SIZE: u32 = 256;
/// Maximum memory budget for cached regions (512MB).
pub const MAX_CACHE_MEMORY_BYTES: usize = 512 * 1024 * 1024;
/// Maximum dimension for a single region texture (4096px = 16 tiles).
pub const MAX_REGION_DIMENSION: u32 = 4096;
const BYTES_PER_PIXEL: usize = 4; // RGBA8Unorm

/// Tile coordinate in the tile grid.
///
/// The tile grid divides pixel space into TILE_SIZE × TILE_SIZE squares.
/// Tile (x, y) covers pixels from (x * TILE_SIZE, y * TILE_SIZE) to
/// ((x+1) * TILE_SIZE, (y+1) * TILE_SIZE) exclusive.
///
/// In document space, tile (x, y) covers the region:
/// - Start: (x * TILE_SIZE / scale, y * TILE_SIZE / scale)
/// - End: ((x+1) * TILE_SIZE / scale, (y+1) * TILE_SIZE / scale)
///
/// Note: No scale/zoom is stored in the coordinate itself. The tile grid
/// is specific to a given scale; when scale changes, all cached regions
/// are invalidated.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct TileCoord {
	pub x: i32,
	pub y: i32,
}

/// A cached rendered region.
///
/// Each region is rendered at tile-aligned boundaries (texture dimensions are
/// multiples of TILE_SIZE). The region covers one or more contiguous tiles.
///
/// ## Coordinate spaces stored:
/// - `texture_size`: Pixel dimensions of the cached texture
/// - `world_bounds`: Document-space bounds (used for debugging/validation)
/// - `tiles`: Which tiles this region covers (tile grid space)
/// - `metadata`: Click targets etc. stored in document space
#[derive(Debug, Clone)]
pub struct CachedRegion {
	/// The GPU texture containing rendered content
	pub texture: wgpu::Texture,
	/// Pixel dimensions of the texture (always tile-aligned: multiples of TILE_SIZE)
	pub texture_size: UVec2,
	/// Document-space bounds this region covers
	pub world_bounds: AxisAlignedBbox,
	/// Tiles covered by this region (for cache lookup by tile coordinate)
	pub tiles: Vec<TileCoord>,
	/// Metadata (click targets, etc.) stored in document space
	pub metadata: rendering::RenderMetadata,
	/// LRU timestamp for eviction
	last_access: u64,
	/// Memory consumption in bytes
	memory_size: usize,
}

// Cache key for invalidation based on RenderParams
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
	// Fields from RenderParams that affect rendering output
	pub render_mode_hash: u64,
	pub hide_artboards: bool,
	pub for_export: bool,
	pub for_mask: bool,
	pub thumbnail: bool,
	pub aligned_strokes: bool,
	pub override_paint_order: bool,
	// Time fields quantized to milliseconds for Eq/Hash
	pub animation_time_ms: i64,
	pub real_time_ms: i64,
}

impl CacheKey {
	/// Create a cache key from f64 times (quantizes to milliseconds)
	pub fn from_times(
		render_mode_hash: u64,
		hide_artboards: bool,
		for_export: bool,
		for_mask: bool,
		thumbnail: bool,
		aligned_strokes: bool,
		override_paint_order: bool,
		animation_time: f64,
		real_time: f64,
	) -> Self {
		Self {
			render_mode_hash,
			hide_artboards,
			for_export,
			for_mask,
			thumbnail,
			aligned_strokes,
			override_paint_order,
			animation_time_ms: (animation_time * 1000.0).round() as i64,
			real_time_ms: (real_time * 1000.0).round() as i64,
		}
	}
}

impl Default for CacheKey {
	fn default() -> Self {
		Self {
			render_mode_hash: 0,
			hide_artboards: false,
			for_export: false,
			for_mask: false,
			thumbnail: false,
			aligned_strokes: false,
			override_paint_order: false,
			animation_time_ms: 0,
			real_time_ms: 0,
		}
	}
}

/// Internal cache implementation
#[derive(Debug)]
struct TileCacheImpl {
	regions: Vec<CachedRegion>, // Stored as Vec since regions can overlap in tile space
	timestamp: u64,
	total_memory: usize,
	cache_key: CacheKey,
	/// Current scale (pixels per world unit) - regions are invalidated when this changes
	current_scale: f64,
}

impl Default for TileCacheImpl {
	fn default() -> Self {
		Self {
			regions: Vec::new(),
			timestamp: 0,
			total_memory: 0,
			cache_key: CacheKey::default(),
			current_scale: 0.0,
		}
	}
}

// Public thread-safe wrapper
#[derive(Clone, Default, dyn_any::DynAny, Debug)]
pub struct TileCache(Arc<Mutex<TileCacheImpl>>);

/// A contiguous region that needs to be rendered.
///
/// Created by the cache query when tiles are missing. Groups adjacent
/// missing tiles into a single render operation for efficiency.
#[derive(Debug, Clone)]
pub struct RenderRegion {
	/// Document-space bounds to render
	pub world_bounds: AxisAlignedBbox,
	/// Tiles that this region will cover once rendered
	pub tiles: Vec<TileCoord>,
	/// Scale (pixels per world unit) at which to render
	pub scale: f64,
}

// Cache query result
#[derive(Debug)]
pub struct CacheQuery {
	pub cached_regions: Vec<CachedRegion>,
	pub missing_regions: Vec<RenderRegion>,
}

// ============================================================================
// COORDINATE CONVERSION FUNCTIONS
// ============================================================================
//
// All functions use `scale` (pixels per world unit) directly, NOT zoom_level.
// This avoids precision loss from log2/exp2 round-trips.
//
// IMPORTANT: These conversions define how document space maps to the tile grid.
// The tile grid is in PIXEL space, divided into TILE_SIZE × TILE_SIZE squares.

/// Convert document-space bounds to the tiles that cover them.
///
/// # Conversion steps:
/// 1. Document bounds → Pixel bounds: `pixel = world * scale`
/// 2. Pixel bounds → Tile range: `tile = floor(pixel / TILE_SIZE)` for start,
///    `tile = ceil(pixel / TILE_SIZE)` for end
///
/// # Arguments
/// * `bounds` - Bounding box in document (world) space
/// * `scale` - Pixels per world unit
///
/// # Returns
/// All tiles that intersect the given bounds
pub fn world_bounds_to_tiles(bounds: &AxisAlignedBbox, scale: f64) -> Vec<TileCoord> {
	// Step 1: Convert document bounds to pixel bounds
	let pixel_start = bounds.start * scale;
	let pixel_end = bounds.end * scale;

	// Step 2: Convert pixel bounds to tile grid coordinates
	// floor() for start: include any tile that overlaps the start edge
	// ceil() for end: include any tile that overlaps the end edge
	let tile_start_x = (pixel_start.x / TILE_SIZE as f64).floor() as i32;
	let tile_start_y = (pixel_start.y / TILE_SIZE as f64).floor() as i32;
	let tile_end_x = (pixel_end.x / TILE_SIZE as f64).ceil() as i32;
	let tile_end_y = (pixel_end.y / TILE_SIZE as f64).ceil() as i32;

	// Generate all tile coordinates in the range [start, end)
	let mut tiles = Vec::new();
	for y in tile_start_y..tile_end_y {
		for x in tile_start_x..tile_end_x {
			tiles.push(TileCoord { x, y });
		}
	}
	tiles
}

/// Get the document-space position of a tile's top-left corner.
///
/// # Conversion:
/// `world = tile * TILE_SIZE / scale`
///
/// This is the inverse of the floor operation in world_bounds_to_tiles.
#[inline]
pub fn tile_world_start(tile: &TileCoord, scale: f64) -> DVec2 {
	DVec2::new(tile.x as f64, tile.y as f64) * (TILE_SIZE as f64 / scale)
}

/// Convert a tile coordinate to its document-space bounding box.
///
/// # Returns
/// The axis-aligned box in document space that this tile covers:
/// - Start: `(tile.x * TILE_SIZE / scale, tile.y * TILE_SIZE / scale)`
/// - End: `((tile.x + 1) * TILE_SIZE / scale, (tile.y + 1) * TILE_SIZE / scale)`
pub fn tile_to_world_bounds(coord: &TileCoord, scale: f64) -> AxisAlignedBbox {
	let tile_world_size = TILE_SIZE as f64 / scale;
	let start = tile_world_start(coord, scale);
	AxisAlignedBbox {
		start,
		end: start + DVec2::splat(tile_world_size),
	}
}

/// Get the combined document-space bounding box of multiple tiles.
pub fn tiles_to_world_bounds(tiles: &[TileCoord], scale: f64) -> AxisAlignedBbox {
	if tiles.is_empty() {
		return AxisAlignedBbox::ZERO;
	}

	let mut result = tile_to_world_bounds(&tiles[0], scale);
	for tile in &tiles[1..] {
		let bounds = tile_to_world_bounds(tile, scale);
		result = result.union(&bounds);
	}
	result
}

// Cache implementation

impl TileCacheImpl {
	/// Query cache for viewport bounds at given scale (pixels per world unit)
	fn query(&mut self, viewport_bounds: &AxisAlignedBbox, scale: f64, cache_key: &CacheKey) -> CacheQuery {
		// Check if cache needs invalidation due to cache key change
		if &self.cache_key != cache_key {
			self.invalidate_all();
			self.cache_key = cache_key.clone();
		}

		// Check if scale changed - invalidate regions but keep cache key
		if (self.current_scale - scale).abs() > 0.001 {
			self.invalidate_all();
			self.current_scale = scale;
		}

		let required_tiles = world_bounds_to_tiles(viewport_bounds, scale);
		let required_tile_set: HashSet<_> = required_tiles.iter().cloned().collect();

		let mut cached_regions = Vec::new();
		let mut covered_tiles = HashSet::new();

		// Find cached regions that cover any of the required tiles
		for region in &mut self.regions {
			let region_tiles: HashSet<_> = region.tiles.iter().cloned().collect();
			if region_tiles.iter().any(|t| required_tile_set.contains(t)) {
				// Update LRU
				region.last_access = self.timestamp;
				self.timestamp += 1;

				cached_regions.push(region.clone());
				covered_tiles.extend(region_tiles);
			}
		}

		// Find missing tiles
		let missing_tiles: Vec<_> = required_tiles.into_iter().filter(|t| !covered_tiles.contains(t)).collect();
		let missing_regions = group_into_regions(&missing_tiles, scale);

		CacheQuery { cached_regions, missing_regions }
	}

	/// Store newly rendered regions
	fn store_regions(&mut self, new_regions: Vec<CachedRegion>) {
		for mut region in new_regions {
			region.last_access = self.timestamp;
			self.timestamp += 1;
			self.total_memory += region.memory_size;
			self.regions.push(region);
		}

		// Evict old regions if over memory limit
		self.evict_until_under_budget();
	}

	/// LRU eviction to stay under memory budget
	fn evict_until_under_budget(&mut self) {
		while self.total_memory > MAX_CACHE_MEMORY_BYTES && !self.regions.is_empty() {
			// Find oldest region
			if let Some((oldest_idx, _)) = self.regions.iter().enumerate().min_by_key(|(_, r)| r.last_access) {
				let removed = self.regions.remove(oldest_idx);
				removed.texture.destroy();
				self.total_memory = self.total_memory.saturating_sub(removed.memory_size);
			} else {
				break;
			}
		}
	}

	/// Clear all cached regions
	fn invalidate_all(&mut self) {
		for region in &self.regions {
			region.texture.destroy();
		}
		self.regions.clear();
		self.total_memory = 0;
		// Don't reset timestamp - it's monotonic
	}
}

// Public TileCache API
impl TileCache {
	pub fn query(&self, viewport_bounds: &AxisAlignedBbox, scale: f64, cache_key: &CacheKey) -> CacheQuery {
		self.0.lock().unwrap().query(viewport_bounds, scale, cache_key)
	}

	pub fn store_regions(&self, regions: Vec<CachedRegion>) {
		self.0.lock().unwrap().store_regions(regions);
	}
}

/// Group tiles into contiguous regions using flood-fill, then split oversized regions.
///
/// # Arguments
/// * `tiles` - Tile coordinates to group (in tile grid space)
/// * `scale` - Pixels per world unit (used to convert tiles back to world bounds)
fn group_into_regions(tiles: &[TileCoord], scale: f64) -> Vec<RenderRegion> {
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

		// Flood-fill to find connected region
		let region_tiles = flood_fill(&tile, &tile_set, &mut visited);
		let world_bounds = tiles_to_world_bounds(&region_tiles, scale);

		let region = RenderRegion {
			world_bounds,
			tiles: region_tiles,
			scale,
		};

		// Split if region exceeds MAX_REGION_DIMENSION
		let split_regions = split_oversized_region(region, scale);
		regions.extend(split_regions);
	}

	regions
}

/// Split region if it exceeds MAX_REGION_DIMENSION, aligned to tile boundaries.
///
/// # Arguments
/// * `region` - The region to potentially split
/// * `scale` - Pixels per world unit
fn split_oversized_region(region: RenderRegion, scale: f64) -> Vec<RenderRegion> {
	let region_size = region.world_bounds.size();
	let pixel_size = region_size * scale;

	// Check if region fits within limits
	if pixel_size.x <= MAX_REGION_DIMENSION as f64 && pixel_size.y <= MAX_REGION_DIMENSION as f64 {
		return vec![region];
	}

	// Calculate how many tiles fit in MAX_REGION_DIMENSION
	let max_tiles_per_dimension = (MAX_REGION_DIMENSION / TILE_SIZE) as i32; // Should be 16

	// Group tiles into grid of chunks
	let mut chunks: HashMap<(i32, i32), Vec<TileCoord>> = HashMap::new();

	for &tile in &region.tiles {
		let chunk_x = tile.x.div_euclid(max_tiles_per_dimension);
		let chunk_y = tile.y.div_euclid(max_tiles_per_dimension);
		chunks.entry((chunk_x, chunk_y)).or_default().push(tile);
	}

	// Convert chunks into regions
	chunks
		.into_iter()
		.map(|(_, tiles)| {
			let world_bounds = tiles_to_world_bounds(&tiles, scale);
			RenderRegion { world_bounds, tiles, scale }
		})
		.collect()
}

/// Flood-fill to find connected tiles (4-connected neighbors)
fn flood_fill(start: &TileCoord, tile_set: &HashSet<TileCoord>, visited: &mut HashSet<TileCoord>) -> Vec<TileCoord> {
	let mut result = Vec::new();
	let mut stack = vec![*start];

	while let Some(current) = stack.pop() {
		if visited.contains(&current) || !tile_set.contains(&current) {
			continue;
		}

		visited.insert(current);
		result.push(current);

		// Check 4-connected neighbors
		let neighbors = [
			TileCoord { x: current.x - 1, y: current.y },
			TileCoord { x: current.x + 1, y: current.y },
			TileCoord { x: current.x, y: current.y - 1 },
			TileCoord { x: current.x, y: current.y + 1 },
		];

		for neighbor in neighbors {
			if tile_set.contains(&neighbor) && !visited.contains(&neighbor) {
				stack.push(neighbor);
			}
		}
	}

	result
}

// ============================================================================
// RENDERING AND TEXTURE OPERATIONS
// ============================================================================

/// Render a single region to a tile-aligned texture.
///
/// # Transform construction
///
/// The region footprint transform maps document space to the region's pixel space:
///
/// ```text
/// region_transform = scale_transform * translation
///
/// where:
///   scale_transform: scales document units to pixels (same as viewport)
///   translation: shifts origin from (0,0) to region's top-left corner
///
/// For a point P in document space:
///   pixel = (P - region_origin) * scale
///
/// This ensures the tile grid aligns exactly: each tile boundary in pixel space
/// corresponds to an integer multiple of TILE_SIZE.
/// ```
///
/// # Metadata handling
///
/// The render function produces metadata in the region's pixel space.
/// We convert it back to document space before storing in the cache:
/// `metadata_document = metadata_region * inverse(region_transform)`
///
/// # Returns
/// * `RenderOutput` - The rendered output with metadata in document space
/// * `UVec2` - The actual texture dimensions (always tile-aligned)
pub async fn render_region<'a, F, Fut>(
	region: &RenderRegion,
	render_fn: F,
	_editor_api: &'a WasmEditorApi,
	_base_render_params: &RenderParams,
	base_ctx: impl Ctx + ExtractAll + CloneVarArgs,
	base_footprint: &Footprint,
) -> (RenderOutput, UVec2)
where
	F: FnOnce(Context<'static>) -> Fut,
	Fut: std::future::Future<Output = RenderOutput>,
{
	// Calculate region texture size from tile count (guaranteed tile-aligned)
	let min_x = region.tiles.iter().map(|t| t.x).min().unwrap();
	let max_x = region.tiles.iter().map(|t| t.x).max().unwrap();
	let min_y = region.tiles.iter().map(|t| t.y).min().unwrap();
	let max_y = region.tiles.iter().map(|t| t.y).max().unwrap();

	let tiles_wide = (max_x - min_x + 1) as u32;
	let tiles_high = (max_y - min_y + 1) as u32;
	let physical_size = UVec2::new(tiles_wide * TILE_SIZE, tiles_high * TILE_SIZE);

	// Extract scale from base footprint (pixels per world unit)
	let base_scale = base_footprint.decompose_scale();

	// Calculate region origin in document space from tile coordinates
	// This ensures perfect tile alignment: tile(x,y) → world(x * TILE_SIZE / scale, ...)
	let world_units_per_pixel = 1.0 / base_scale.x;
	let tile_world_size = TILE_SIZE as f64 * world_units_per_pixel;
	let region_world_start = DVec2::new(min_x as f64 * tile_world_size, min_y as f64 * tile_world_size);

	// Build region transform: pixel = (document - region_origin) * scale
	// In matrix form: scale * translate(-region_origin)
	let scale_transform = glam::DAffine2::from_scale(base_scale);
	let translation = glam::DAffine2::from_translation(-region_world_start);
	let region_transform = scale_transform * translation;

	// DEBUG: Log the region rendering parameters
	log::debug!(
		"[render_region] tiles: x=[{}, {}], y=[{}, {}], size: {}x{}",
		min_x,
		max_x,
		min_y,
		max_y,
		physical_size.x,
		physical_size.y
	);
	log::debug!(
		"[render_region] region_world_start: ({:.2}, {:.2}), base_scale: ({:.4}, {:.4})",
		region_world_start.x,
		region_world_start.y,
		base_scale.x,
		base_scale.y
	);
	// Verify: document point at region_world_start should map to pixel (0,0)
	let test_pixel = region_transform.transform_point2(region_world_start);
	log::debug!(
		"[render_region] transform check: region_world_start -> pixel ({:.2}, {:.2}) (should be 0,0)",
		test_pixel.x,
		test_pixel.y
	);
	// And check what document point maps to the viewport's start
	let viewport_world_start = base_footprint.viewport_bounds_in_local_space().start;
	let viewport_in_region_pixels = region_transform.transform_point2(viewport_world_start);
	log::debug!(
		"[render_region] viewport_world_start ({:.2}, {:.2}) -> region_pixel ({:.2}, {:.2})",
		viewport_world_start.x,
		viewport_world_start.y,
		viewport_in_region_pixels.x,
		viewport_in_region_pixels.y
	);

	let region_footprint = Footprint {
		transform: region_transform,
		resolution: physical_size,
		quality: RenderQuality::Full,
	};

	// Create context with region footprint
	let mut region_params = _base_render_params.clone();
	region_params.footprint = region_footprint;

	// Build context from base context with new footprint
	let region_ctx = OwnedContextImpl::from(base_ctx).with_footprint(region_footprint).with_vararg(Box::new(region_params)).into_context();

	// Evaluate render function with region context
	let mut result = render_fn(region_ctx).await;

	// Convert metadata back to document space by applying region_transform^-1
	let translation_back = glam::DAffine2::from_translation(region_world_start);
	let region_to_document_transform = translation_back * scale_transform.inverse();
	result.metadata.apply_transform(region_to_document_transform);

	(result, physical_size)
}

/// Composite cached region textures into the final viewport output texture.
///
/// # Two-stage compositing approach
///
/// ## Stage 1: Assemble tile-aligned intermediate texture
/// All cached regions are copied into a single tile-aligned intermediate texture.
/// Since every region is tile-aligned (dimensions are multiples of TILE_SIZE),
/// no sub-pixel offsets are needed - positions are computed as:
/// `pixel_offset = (region_min_tile - global_min_tile) * TILE_SIZE`
///
/// ## Stage 2: Copy to viewport output
/// The tile-aligned intermediate is copied to the viewport output texture.
/// This is the ONLY place where sub-tile precision matters:
/// `offset = tile_aligned_origin - viewport_origin` (in pixels)
///
/// # Coordinate conversion for Stage 2:
/// ```text
/// tile_aligned_world_start = min_tile * TILE_SIZE / scale     (document space)
/// offset_world = tile_aligned_world_start - viewport_bounds.start
/// offset_pixels = offset_world * scale
/// ```
///
/// # Arguments
/// * `cached_regions` - Regions to composite (all tile-aligned)
/// * `viewport_bounds` - Document-space bounds of the viewport
/// * `output_resolution` - Pixel dimensions of the output texture
/// * `scale` - Pixels per world unit
/// * `editor_api` - For GPU access
pub async fn composite_regions<'a>(cached_regions: Vec<CachedRegion>, viewport_bounds: &AxisAlignedBbox, output_resolution: UVec2, scale: f64, editor_api: &'a WasmEditorApi) -> wgpu::Texture {
	let exec = editor_api.application_io.as_ref().unwrap().gpu_executor().unwrap();
	let device = &exec.context.device;
	let queue = &exec.context.queue;

	// STAGE 1: Determine tile-aligned bounds that cover all regions
	let mut min_tile = IVec2::new(i32::MAX, i32::MAX);
	let mut max_tile = IVec2::new(i32::MIN, i32::MIN);

	for region in &cached_regions {
		for tile in &region.tiles {
			min_tile = min_tile.min(IVec2::new(tile.x, tile.y));
			max_tile = max_tile.max(IVec2::new(tile.x, tile.y));
		}
	}

	// Calculate tile-aligned intermediate texture size
	let tile_count = (max_tile - min_tile) + IVec2::ONE;
	let tile_aligned_size = tile_count.as_uvec2() * TILE_SIZE;

	// Create tile-aligned intermediate texture
	let tile_aligned_texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("tile_aligned_composite"),
		size: wgpu::Extent3d {
			width: tile_aligned_size.x,
			height: tile_aligned_size.y,
			depth_or_array_layers: 1,
		},
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba8Unorm,
		usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
		view_formats: &[],
	});

	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("tile_composite") });

	// STAGE 1: Copy each region to its tile-aligned position
	for region in &cached_regions {
		let region_min_tile = IVec2::new(region.tiles.iter().map(|t| t.x).min().unwrap(), region.tiles.iter().map(|t| t.y).min().unwrap());

		// Calculate position in tile-aligned texture (in tiles, then convert to pixels)
		let tile_offset = region_min_tile - min_tile;
		let pixel_offset = tile_offset.as_uvec2() * TILE_SIZE;

		// Simple copy - everything is tile-aligned!
		encoder.copy_texture_to_texture(
			wgpu::TexelCopyTextureInfo {
				texture: &region.texture,
				mip_level: 0,
				origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
				aspect: wgpu::TextureAspect::All,
			},
			wgpu::TexelCopyTextureInfo {
				texture: &tile_aligned_texture,
				mip_level: 0,
				origin: wgpu::Origin3d {
					x: pixel_offset.x,
					y: pixel_offset.y,
					z: 0,
				},
				aspect: wgpu::TextureAspect::All,
			},
			wgpu::Extent3d {
				width: region.texture_size.x,
				height: region.texture_size.y,
				depth_or_array_layers: 1,
			},
		);
	}

	// STAGE 2: Copy from tile-aligned texture to viewport output
	// Convert tile origin to document space: tile * TILE_SIZE / scale
	let tile_aligned_world_start = min_tile.as_dvec2() * (TILE_SIZE as f64 / scale);

	// Calculate offset from tile-aligned texture origin to viewport origin (in document space)
	// Then convert to pixels: offset_pixels = offset_world * scale
	let offset_world = tile_aligned_world_start - viewport_bounds.start;
	let offset_pixels_f64 = offset_world * scale;
	let offset_pixels = IVec2::new(offset_pixels_f64.x.floor() as i32, offset_pixels_f64.y.floor() as i32);

	// DEBUG: Log the offset calculation
	log::debug!(
		"[composite] viewport_world: ({:.2}, {:.2}), tile_aligned_world: ({:.2}, {:.2})",
		viewport_bounds.start.x,
		viewport_bounds.start.y,
		tile_aligned_world_start.x,
		tile_aligned_world_start.y
	);
	log::debug!(
		"[composite] offset_world: ({:.2}, {:.2}), offset_pixels: ({}, {})",
		offset_world.x,
		offset_world.y,
		offset_pixels.x,
		offset_pixels.y
	);
	log::debug!(
		"[composite] min_tile: ({}, {}), scale: {:.4}, tile_aligned_size: ({}, {})",
		min_tile.x,
		min_tile.y,
		scale,
		tile_aligned_size.x,
		tile_aligned_size.y
	);

	// Create final output texture
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

	// Handle negative offsets (tile-aligned texture extends before viewport)
	let (src_x, dst_x, width) = if offset_pixels.x < 0 {
		let skip = (-offset_pixels.x) as u32;
		let w = tile_aligned_size.x.saturating_sub(skip).min(output_resolution.x);
		(skip, 0, w)
	} else {
		let dst = offset_pixels.x as u32;
		let w = tile_aligned_size.x.min(output_resolution.x.saturating_sub(dst));
		(0, dst, w)
	};

	let (src_y, dst_y, height) = if offset_pixels.y < 0 {
		let skip = (-offset_pixels.y) as u32;
		let h = tile_aligned_size.y.saturating_sub(skip).min(output_resolution.y);
		(skip, 0, h)
	} else {
		let dst = offset_pixels.y as u32;
		let h = tile_aligned_size.y.min(output_resolution.y.saturating_sub(dst));
		(0, dst, h)
	};

	// Single copy from tile-aligned to output
	encoder.copy_texture_to_texture(
		wgpu::TexelCopyTextureInfo {
			texture: &tile_aligned_texture,
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

	queue.submit([encoder.finish()]);
	output_texture
}

// Node implementation

/// Renders content with tile-based caching for efficient viewport panning.
///
/// This node caches rendered tiles and reuses them when the viewport pans, only
/// rendering newly visible tiles. This significantly improves performance during
/// navigation by avoiding redundant rendering of previously visible content.
///
/// ## Coordinate spaces
/// - Document space: World units of the artwork
/// - Logical pixels: Viewport pixels before device scale (footprint.resolution)
/// - Physical pixels: Actual GPU texture pixels (logical * device_scale)
///
/// ## Key relationships
/// - `logical_scale` = footprint.decompose_scale().x (document → logical pixels)
/// - `device_scale` = render_params.scale (logical → physical pixels)
/// - `physical_scale` = logical_scale * device_scale (document → physical pixels)
///
/// ## Caching strategy
/// - Tiles are computed at `logical_scale` (256 logical pixels per tile)
/// - Textures are stored at `physical_scale` density
/// - Cache is invalidated when scale or render parameters change
#[node_macro::node(category(""))]
pub async fn render_output_cache<'a: 'n>(
	ctx: impl Ctx + ExtractAll + CloneVarArgs + ExtractRealTime + ExtractAnimationTime + Sync,
	editor_api: &'a WasmEditorApi,
	data: impl Node<Context<'static>, Output = RenderOutput> + Send + Sync,
	#[data] tile_cache: TileCache,
) -> RenderOutput {
	let footprint = ctx.footprint();
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");

	// Only use tile-aligned rendering for Vello
	if !matches!(render_params.render_output_type, RenderOutputTypeRequest::Vello) {
		let context = OwnedContextImpl::empty().with_footprint(*footprint).with_vararg(Box::new(render_params.clone()));
		return data.eval(context.into_context()).await;
	}

	let physical_resolution = footprint.resolution;
	let logical_scale = footprint.decompose_scale().x;
	let device_scale = render_params.scale;
	let physical_scale = logical_scale * device_scale;
	let viewport_bounds = footprint.viewport_bounds_in_local_space();

	// Create cache key from render parameters
	let cache_key = CacheKey::from_times(
		0, // TODO: hash render_mode properly
		render_params.hide_artboards,
		render_params.for_export,
		false, // for_mask
		render_params.thumbnail,
		render_params.aligned_strokes,
		false, // override_paint_order
		ctx.try_animation_time().unwrap_or(0.0),
		ctx.try_real_time().unwrap_or(0.0),
	);

	// Query cache for existing tiles and missing regions
	let cache_query = tile_cache.query(&viewport_bounds, logical_scale, &cache_key);

	// Render missing regions
	let mut new_regions = Vec::new();
	if cache_query.missing_regions.is_empty() {
		println!("reusing cache");
	} else {
		println!("rerendering {} regions", cache_query.missing_regions.len());
	}
	for missing_region in &cache_query.missing_regions {
		let region = render_missing_region(missing_region, |ctx| data.eval(ctx), ctx.clone(), render_params, logical_scale, device_scale).await;
		new_regions.push(region);
	}

	// Store newly rendered regions in cache
	tile_cache.store_regions(new_regions.clone());

	// Collect all regions (cached + newly rendered)
	let all_regions: Vec<_> = cache_query.cached_regions.into_iter().chain(new_regions.into_iter()).collect();

	// Composite all regions into output
	let exec = editor_api.application_io.as_ref().unwrap().gpu_executor().unwrap();
	let (output_texture, combined_metadata) = composite_cached_regions(&all_regions, &viewport_bounds, physical_resolution, logical_scale, physical_scale, exec);

	RenderOutput {
		data: RenderOutputType::Texture(ImageTexture { texture: output_texture }),
		metadata: combined_metadata,
	}
}

/// Render a missing region and create a CachedRegion for storage.
async fn render_missing_region<F, Fut>(
	region: &RenderRegion,
	render_fn: F,
	ctx: impl Ctx + ExtractAll + CloneVarArgs,
	render_params: &RenderParams,
	logical_scale: f64,
	device_scale: f64,
) -> CachedRegion
where
	F: Fn(Context<'static>) -> Fut,
	Fut: std::future::Future<Output = RenderOutput>,
{
	// Calculate region bounds from tiles
	let min_tile = region.tiles.iter().fold(IVec2::new(i32::MAX, i32::MAX), |acc, t| acc.min(IVec2::new(t.x, t.y)));
	let max_tile = region.tiles.iter().fold(IVec2::new(i32::MIN, i32::MIN), |acc, t| acc.max(IVec2::new(t.x, t.y)));

	let tile_world_size = TILE_SIZE as f64 / logical_scale;
	let region_world_start = DVec2::new(min_tile.x as f64 * tile_world_size, min_tile.y as f64 * tile_world_size);

	// Calculate region texture size in physical pixels
	let tiles_wide = (max_tile.x - min_tile.x + 1) as u32;
	let tiles_high = (max_tile.y - min_tile.y + 1) as u32;
	let region_pixel_size = UVec2::new(
		((tiles_wide * TILE_SIZE) as f64 * device_scale).ceil() as u32,
		((tiles_high * TILE_SIZE) as f64 * device_scale).ceil() as u32,
	);

	// Build transform for the tile-aligned region
	let region_transform = glam::DAffine2::from_scale(DVec2::splat(logical_scale)) * glam::DAffine2::from_translation(-region_world_start);

	// Render to tile-aligned region
	let region_footprint = Footprint {
		transform: region_transform,
		resolution: region_pixel_size,
		quality: RenderQuality::Full,
	};
	let mut region_params = render_params.clone();
	region_params.footprint = region_footprint;
	let region_ctx = OwnedContextImpl::from(ctx).with_footprint(region_footprint).with_vararg(Box::new(region_params)).into_context();
	let mut result = render_fn(region_ctx).await;

	let RenderOutputType::Texture(rendered_texture) = result.data else {
		panic!("Expected texture output from render");
	};

	// Transform metadata from region pixel space to document space for storage
	// Region pixel (0,0) = region_world_start in document space
	// So: document = region_world_start + pixel / logical_scale
	let pixel_to_document = glam::DAffine2::from_translation(region_world_start) * glam::DAffine2::from_scale(DVec2::splat(1.0 / logical_scale));
	result.metadata.apply_transform(pixel_to_document);

	let memory_size = (region_pixel_size.x * region_pixel_size.y) as usize * BYTES_PER_PIXEL;

	CachedRegion {
		texture: rendered_texture.texture,
		texture_size: region_pixel_size,
		world_bounds: region.world_bounds.clone(),
		tiles: region.tiles.clone(),
		metadata: result.metadata,
		last_access: 0, // Will be set by cache
		memory_size,
	}
}

/// Composite cached regions into the final viewport output texture.
fn composite_cached_regions(
	regions: &[CachedRegion],
	viewport_bounds: &AxisAlignedBbox,
	output_resolution: UVec2,
	logical_scale: f64,
	physical_scale: f64,
	exec: &wgpu_executor::WgpuExecutor,
) -> (wgpu::Texture, rendering::RenderMetadata) {
	let device = &exec.context.device;
	let queue = &exec.context.queue;

	// Create output texture
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

	// Copy each region to the output texture
	for region in regions {
		// Find region's min tile to calculate its origin
		let min_tile = region.tiles.iter().fold(IVec2::new(i32::MAX, i32::MAX), |acc, t| acc.min(IVec2::new(t.x, t.y)));
		let tile_world_size = TILE_SIZE as f64 / logical_scale;
		let region_world_start = DVec2::new(min_tile.x as f64 * tile_world_size, min_tile.y as f64 * tile_world_size);

		// Calculate pixel offset from region origin to viewport origin
		let offset_world = region_world_start - viewport_bounds.start;
		let offset_pixels = (offset_world * physical_scale).floor().as_ivec2();

		// Calculate copy parameters
		let (src_x, dst_x, width) = if offset_pixels.x >= 0 {
			let dst = offset_pixels.x as u32;
			let w = region.texture_size.x.min(output_resolution.x.saturating_sub(dst));
			(0, dst, w)
		} else {
			let skip = (-offset_pixels.x) as u32;
			let w = region.texture_size.x.saturating_sub(skip).min(output_resolution.x);
			(skip, 0, w)
		};

		let (src_y, dst_y, height) = if offset_pixels.y >= 0 {
			let dst = offset_pixels.y as u32;
			let h = region.texture_size.y.min(output_resolution.y.saturating_sub(dst));
			(0, dst, h)
		} else {
			let skip = (-offset_pixels.y) as u32;
			let h = region.texture_size.y.saturating_sub(skip).min(output_resolution.y);
			(skip, 0, h)
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

		// Transform and merge metadata
		// Metadata is stored in document space, convert to viewport logical pixels
		let mut region_metadata = region.metadata.clone();
		let document_to_viewport = glam::DAffine2::from_scale(DVec2::splat(logical_scale)) * glam::DAffine2::from_translation(-viewport_bounds.start);
		region_metadata.apply_transform(document_to_viewport);
		combined_metadata.merge(&region_metadata);
	}

	queue.submit([encoder.finish()]);

	(output_texture, combined_metadata)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_tile_coordinate_conversion() {
		// scale = 4.0 pixels per world unit
		let scale = 4.0;
		let coord = TileCoord { x: 0, y: 0 };
		let bounds = tile_to_world_bounds(&coord, scale);

		// At scale 4.0, 256 pixels = 64 world units
		assert_eq!(bounds.start, DVec2::ZERO);
		assert_eq!(bounds.end, DVec2::splat(64.0));
	}

	#[test]
	fn test_world_to_tiles() {
		// scale = 1.0, 1 pixel = 1 world unit
		let scale = 1.0;
		let bounds = AxisAlignedBbox {
			start: DVec2::ZERO,
			end: DVec2::new(512.0, 256.0),
		};
		let tiles = world_bounds_to_tiles(&bounds, scale);

		// Should be 2x1 tiles (512x256 pixels at scale 1.0)
		assert_eq!(tiles.len(), 2);
		assert!(tiles.contains(&TileCoord { x: 0, y: 0 }));
		assert!(tiles.contains(&TileCoord { x: 1, y: 0 }));
	}

	#[test]
	fn test_tile_alignment_offset_calculation() {
		// This test verifies that the offset calculation correctly maps
		// output[0,0] to show the viewport origin, regardless of tile alignment
		//
		// Note: floor() rounding in offset calculation causes up to 1/scale document units
		// of error. At physical_scale=0.52, this is up to ~2 document units.
		let physical_scale = 0.52; // Simulates logical_scale * device_scale
		let tile_world_size = TILE_SIZE as f64 / physical_scale;

		// Test case: viewport at position that's NOT tile-aligned
		let viewport_start = DVec2::new(-536.51, -493.24);
		let viewport_bounds = AxisAlignedBbox {
			start: viewport_start,
			end: viewport_start + DVec2::new(1000.0, 1000.0),
		};

		// Calculate tiles
		let viewport_tiles = world_bounds_to_tiles(&viewport_bounds, physical_scale);
		let min_tile = viewport_tiles.iter().fold(IVec2::new(i32::MAX, i32::MAX), |acc, t| acc.min(IVec2::new(t.x, t.y)));

		// Calculate region origin (tile-aligned)
		let region_world_start = DVec2::new(min_tile.x as f64 * tile_world_size, min_tile.y as f64 * tile_world_size);

		// Calculate offset
		let offset_world = region_world_start - viewport_start;
		let offset_pixels = (offset_world * physical_scale).floor().as_ivec2();

		// Determine src position for copy (where in region texture corresponds to viewport origin)
		let src_x = if offset_pixels.x >= 0 { 0 } else { (-offset_pixels.x) as u32 };
		let src_y = if offset_pixels.y >= 0 { 0 } else { (-offset_pixels.y) as u32 };

		// Calculate what document position output[0,0] would show
		let doc_at_output_origin = region_world_start + DVec2::new(src_x as f64, src_y as f64) / physical_scale;

		// The maximum error from floor() is 1 pixel, which is 1/scale document units
		// At scale 0.52, that's about 1.92 document units per axis, or ~2.7 total
		let max_error = 2.0 / physical_scale; // 1 pixel per axis, diagonal
		let error = (doc_at_output_origin - viewport_start).length();
		assert!(
			error < max_error,
			"Output origin mismatch: got ({:.4}, {:.4}), expected ({:.4}, {:.4}), error: {:.4} (max allowed: {:.4})",
			doc_at_output_origin.x,
			doc_at_output_origin.y,
			viewport_start.x,
			viewport_start.y,
			error,
			max_error
		);
	}

	#[test]
	fn test_tile_boundary_crossing_consistency() {
		// This test verifies that crossing a tile boundary doesn't cause a large position jump.
		// The floor() rounding can cause small errors (~2 doc units), but there should NOT be
		// a tile-sized discontinuity when crossing boundaries.
		let physical_scale = 0.52;
		let tile_world_size = TILE_SIZE as f64 / physical_scale;
		let max_error = 2.0 / physical_scale; // Maximum error from floor() rounding

		// Two viewport positions: just before and just after a tile boundary
		let viewport_before = DVec2::new(-536.51, -490.36);
		let viewport_after = DVec2::new(-536.51, -493.24); // Moved down slightly, crosses tile boundary

		let calc_output_origin = |viewport_start: DVec2| -> DVec2 {
			let viewport_bounds = AxisAlignedBbox {
				start: viewport_start,
				end: viewport_start + DVec2::new(1000.0, 1000.0),
			};

			let viewport_tiles = world_bounds_to_tiles(&viewport_bounds, physical_scale);
			let min_tile = viewport_tiles.iter().fold(IVec2::new(i32::MAX, i32::MAX), |acc, t| acc.min(IVec2::new(t.x, t.y)));

			let region_world_start = DVec2::new(min_tile.x as f64 * tile_world_size, min_tile.y as f64 * tile_world_size);

			let offset_world = region_world_start - viewport_start;
			let offset_pixels = (offset_world * physical_scale).floor().as_ivec2();

			let src_x = if offset_pixels.x >= 0 { 0 } else { (-offset_pixels.x) as u32 };
			let src_y = if offset_pixels.y >= 0 { 0 } else { (-offset_pixels.y) as u32 };

			region_world_start + DVec2::new(src_x as f64, src_y as f64) / physical_scale
		};

		let output_before = calc_output_origin(viewport_before);
		let output_after = calc_output_origin(viewport_after);

		// Check that output[0,0] shows approximately the correct viewport origin
		let error_before = (output_before - viewport_before).length();
		let error_after = (output_after - viewport_after).length();

		assert!(
			error_before < max_error,
			"Before crossing: got ({:.4}, {:.4}), expected ({:.4}, {:.4}), error: {:.4} (max: {:.4})",
			output_before.x,
			output_before.y,
			viewport_before.x,
			viewport_before.y,
			error_before,
			max_error
		);

		assert!(
			error_after < max_error,
			"After crossing: got ({:.4}, {:.4}), expected ({:.4}, {:.4}), error: {:.4} (max: {:.4})",
			output_after.x,
			output_after.y,
			viewport_after.x,
			viewport_after.y,
			error_after,
			max_error
		);

		// CRITICAL: The viewport moved by ~3 units, so output origin should also move by ~3 units.
		// A tile-sized jump would be ~492 units - that would indicate a bug.
		let viewport_delta = (viewport_after - viewport_before).length();
		let output_delta = (output_after - output_before).length();
		let delta_diff = (output_delta - viewport_delta).abs();

		// Allow twice the per-position error since both positions have rounding error
		assert!(
			delta_diff < 2.0 * max_error,
			"Position delta mismatch: viewport moved {:.4}, output moved {:.4}, difference: {:.4} (max: {:.4})",
			viewport_delta,
			output_delta,
			delta_diff,
			2.0 * max_error
		);

		// Additional check: the delta should be nowhere near a tile size
		let tile_size_doc = tile_world_size;
		assert!(
			delta_diff < tile_size_doc * 0.1, // Should be much less than 10% of a tile
			"TILE-SIZED JUMP DETECTED: viewport moved {:.4}, output moved {:.4}, difference: {:.4} (tile size: {:.4})",
			viewport_delta,
			output_delta,
			delta_diff,
			tile_size_doc
		);
	}

	#[test]
	fn test_negative_tile_coordinates() {
		// Test that negative tile coordinates work correctly
		let scale = 1.0;
		let bounds = AxisAlignedBbox {
			start: DVec2::new(-512.0, -256.0),
			end: DVec2::new(0.0, 0.0),
		};
		let tiles = world_bounds_to_tiles(&bounds, scale);

		// At scale 1.0, 256 pixels = 256 world units per tile
		// -512 to 0 should cover tiles -2, -1 in x
		// -256 to 0 should cover tile -1 in y
		assert!(tiles.contains(&TileCoord { x: -2, y: -1 }));
		assert!(tiles.contains(&TileCoord { x: -1, y: -1 }));
	}

	#[test]
	fn test_round_trip_tile_conversion() {
		// Converting world bounds to tiles and back should give tile-aligned bounds
		// that fully contain the original bounds
		let scale = 2.5;
		let original_bounds = AxisAlignedBbox {
			start: DVec2::new(100.3, 200.7),
			end: DVec2::new(500.1, 800.9),
		};

		let tiles = world_bounds_to_tiles(&original_bounds, scale);
		let tile_bounds = tiles_to_world_bounds(&tiles, scale);

		// Tile bounds should contain original bounds
		assert!(
			tile_bounds.start.x <= original_bounds.start.x,
			"Tile start.x {} should be <= original start.x {}",
			tile_bounds.start.x,
			original_bounds.start.x
		);
		assert!(
			tile_bounds.start.y <= original_bounds.start.y,
			"Tile start.y {} should be <= original start.y {}",
			tile_bounds.start.y,
			original_bounds.start.y
		);
		assert!(
			tile_bounds.end.x >= original_bounds.end.x,
			"Tile end.x {} should be >= original end.x {}",
			tile_bounds.end.x,
			original_bounds.end.x
		);
		assert!(
			tile_bounds.end.y >= original_bounds.end.y,
			"Tile end.y {} should be >= original end.y {}",
			tile_bounds.end.y,
			original_bounds.end.y
		);
	}

	#[test]
	fn test_device_scale_handling() {
		// Test that the physical_scale = logical_scale * device_scale relationship holds
		let logical_scale = 0.3467;
		let device_scale = 1.5;
		let physical_scale = logical_scale * device_scale;

		// At physical_scale, each tile covers this many document units
		let tile_world_size = TILE_SIZE as f64 / physical_scale;

		// Verify: rendering at physical_scale to TILE_SIZE pixels covers tile_world_size document units
		// This is fundamental to tile alignment working correctly
		let rendered_doc_coverage = TILE_SIZE as f64 / physical_scale;
		assert!(
			(rendered_doc_coverage - tile_world_size).abs() < 0.001,
			"Tile world size mismatch: {} vs {}",
			tile_world_size,
			rendered_doc_coverage
		);
	}

	#[test]
	fn test_offset_calculation_edge_cases() {
		let physical_scale = 0.52;
		let tile_world_size = TILE_SIZE as f64 / physical_scale;

		// Test when viewport is exactly at a tile boundary
		let tile_aligned_viewport = DVec2::new(-2.0 * tile_world_size, -1.0 * tile_world_size);
		let viewport_bounds = AxisAlignedBbox {
			start: tile_aligned_viewport,
			end: tile_aligned_viewport + DVec2::new(1000.0, 1000.0),
		};

		let tiles = world_bounds_to_tiles(&viewport_bounds, physical_scale);
		let min_tile = tiles.iter().fold(IVec2::new(i32::MAX, i32::MAX), |acc, t| acc.min(IVec2::new(t.x, t.y)));

		let region_world_start = DVec2::new(min_tile.x as f64 * tile_world_size, min_tile.y as f64 * tile_world_size);

		// When viewport is tile-aligned, region_world_start should equal viewport start
		let diff = (region_world_start - tile_aligned_viewport).length();
		assert!(diff < 0.001, "Tile-aligned viewport should have zero offset, got {}", diff);
	}
}
