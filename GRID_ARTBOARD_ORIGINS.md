# Grid Origins for Artboards

## Problem

Previously, the grid system in Graphite used a single global origin point (0,0) for all grids, which caused problems when working with multiple artboards, especially for isometric grids. When users had multiple artboards in their document, the grid lines would originate from the document's (0,0) point rather than from each artboard's origin, making it difficult to create grid-aligned artwork within specific artboards.

## Solution

The grid system has been enhanced to support artboard-specific origins with user-configurable origin modes. Here's how it works:

### Grid Origin Selection Logic

The grid origin behavior is now controlled by the **Origin Mode** setting:

1. **Global mode**: The grid always uses the global grid origin (configurable in the grid options panel)
2. **Artboard mode** (default): The grid uses the origin point (top-left corner) of each selected artboard, falling back to the global origin when no artboards are selected

### User Interface

The grid options panel now includes an **Origin Mode** selector with two options:
- **Global**: Forces the grid to always use the global origin point
- **Artboard**: Uses selected artboard origins when available, otherwise falls back to global origin

This gives users explicit control over grid behavior without needing to understand the implicit selection-based behavior.

### Implementation Details

#### Grid Overlay Rendering
- `get_grid_origins()` function determines which origins to use
- Grid overlay functions (`grid_overlay_rectangular`, `grid_overlay_isometric`, etc.) now iterate over multiple origins
- Each origin generates its own set of grid lines within the viewport
- **Performance optimization**: Duplicate or very close origins are automatically filtered out to improve rendering performance

#### Grid Snapping
- Grid snapping also uses the same origin selection logic  
- `GridSnapper::get_grid_origins()` method provides consistent origin selection
- Both rectangular and isometric grid snapping support multiple origins
- **Performance optimization**: Duplicate or very close origins are automatically filtered out to improve snapping performance

### Code Changes

The main changes were made in:

1. **`grid_overlays.rs`**:
   - Added `get_grid_origins()` function to determine active grid origins
   - Modified all grid overlay functions to use multiple origins
   - Grid lines are now generated for each active origin

2. **`grid_snapper.rs`**:
   - Added `get_grid_origins()` method to `GridSnapper`
   - Modified `get_snap_lines_rectangular()` and `get_snap_lines_isometric()` to generate snap lines for multiple origins
   - Grid snapping now works correctly with artboard-specific origins

### User Experience

- **Default behavior**: When no artboards are selected, grids work exactly as before
- **Artboard-specific grids**: When artboards are selected, grids originate from each artboard's corner
- **Multiple artboards**: Users can select multiple artboards to see grids for all of them simultaneously
- **Backward compatibility**: All existing grid functionality remains unchanged

### Usage Examples

#### Using Global Origin Mode
1. Open the grid options panel (click the grid icon in the viewport controls)
2. Set **Origin Mode** to **Global**
3. The grid will always originate from the global origin point, regardless of artboard selection

#### Using Artboard Origin Mode (Default)
1. Open the grid options panel (click the grid icon in the viewport controls)
2. Set **Origin Mode** to **Artboard** (this is the default)
3. Create multiple artboards in your document
4. Select one or more artboards using the selection tool
5. Enable grid display in the viewport options
6. The grid will now originate from the corner of each selected artboard

#### Mixed Workflow
Users can switch between modes as needed:
- Use **Global** mode for document-wide grid alignment
- Use **Artboard** mode for artboard-specific grid alignment
- Switch between modes without losing grid settings

This enhancement provides more flexibility for users working with multiple artboards while maintaining backward compatibility with existing workflows.
