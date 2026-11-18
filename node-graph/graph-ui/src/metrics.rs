use glam::DVec2;

pub const GRID_SIZE: f64 = 24.;
pub const GRID_COLLAPSE_SPACING: f64 = 10.;
pub const TEXT_SIZE: f64 = 14.;
pub const BORDER_WIDTH: f64 = 1.;
pub const BORDER_DASHES: [f64; 2] = [3., 2.];
pub const ICON_SIZE: f64 = 16.;

pub const NODE_WIDTH: f64 = 5. * GRID_SIZE;
pub const NODE_RADIUS: f64 = 2.;
pub const NODE_ICON_LEFT: f64 = 8.;
pub const NODE_NAME_LEFT: f64 = 32.;
pub const NODE_NAME_MAX_WIDTH: f64 = 84.;
pub const NODE_ROW_PADDING: f64 = 8.;

pub const CONNECTOR_SIZE: f64 = 8.;
pub const INPUT_CONNECTOR_LEFT: f64 = -3.;
pub const OUTPUT_CONNECTOR_RIGHT: f64 = 5.;
pub const CONNECTOR_GAP: f64 = 16.;
pub const HOVERED_CONNECTOR_SCALE: f64 = 1.3;
pub const HOVERED_CONNECTOR_RING: f64 = 4.;
pub const STACK_CONNECTOR_SIZE: DVec2 = DVec2::new(CONNECTOR_SIZE, 12.);

pub const IMPORT_EXPORT_CONNECTOR: DVec2 = DVec2::new(INPUT_CONNECTOR_LEFT, -CONNECTOR_SIZE / 2.);
pub const IMPORT_NAME_INSET: f64 = 8.;
pub const EXPORT_NAME_INSET: f64 = 9.;
pub const IMPORT_EXPORT_NAME_RAISE: f64 = 1.;
pub const IMPORT_EXPORT_SEPARATOR_OVERHANG: f64 = 4.;
pub const REORDER_BAR_SIZE: DVec2 = DVec2::new(50., 2.);

pub const ERROR_PADDING: f64 = 8.;
pub const ERROR_LINE_HEIGHT: f64 = 18.;
pub const ERROR_MAX_WIDTH: f64 = 600.;
pub const ERROR_RADIUS: f64 = 4.;
pub const ERROR_TAIL_LEFT: f64 = 6.;
pub const ERROR_TAIL_SIZE: DVec2 = DVec2::new(12., 8.);

pub const WIRE_WIDTH: f64 = 2.;
pub const LIST_WIRE_WIDTH: f64 = 4.;
pub const WIRE_DASHES: [f64; 2] = [3., 2.];
pub const THICK_WIRE_WIDTH: f64 = 8.;

pub const DOT_RADIUS: f64 = 5.;
pub const DOT_SELECTED_RING: f64 = 2.;
pub const DOT_PREVIEWED_RING_OFFSET: f64 = 2.;
pub const DOT_HOVERED_RING: f64 = 2.;
pub const DOT_OUTPUT_ARROW: DVec2 = DVec2::new(6., -4.);

pub const LAYER_HEIGHT: f64 = 2. * GRID_SIZE;
pub const LAYER_RADIUS: f64 = 8.;
pub const LAYER_LEFT_MARGIN: f64 = GRID_SIZE / 2.;
pub const LAYER_THUMBNAIL_WIDTH: f64 = 3. * GRID_SIZE;
pub const LAYER_THUMBNAIL_MARGIN: f64 = 1.;
pub const LAYER_THUMBNAIL_RADIUS: f64 = 2.;
pub const LAYER_THUMBNAIL_IMAGE_INSET: f64 = 2.;
pub const CHECKER_CELL: f64 = 8.;
pub const LAYER_DETAILS_MARGIN: f64 = 8.;
pub const LAYER_NAME_LEFT: f64 = LAYER_LEFT_MARGIN + LAYER_THUMBNAIL_WIDTH + LAYER_DETAILS_MARGIN;
pub const LAYER_BUTTON_SIZE: f64 = GRID_SIZE;
pub const LAYER_BUTTON_RADIUS: f64 = 2.;
pub const LAYER_GRIP_WIDTH: f64 = 8.;
pub const LAYER_GRIP_DOTS: DVec2 = DVec2::new(2.5, 9.5);
pub const LAYER_GRIP_DOT_COUNT: [u32; 2] = [2, 3];
pub const LAYER_GRIP_DOT_SPACING: f64 = 3.;
pub const LAYER_GRIP_DOT_RADIUS: f64 = 0.5;
pub const LAYER_THUMBNAIL_GAP: DVec2 = DVec2::new(8., 2.);
