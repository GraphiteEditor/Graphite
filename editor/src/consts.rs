// GRAPH
pub const GRID_SIZE: u32 = 24;
pub const EXPORTS_TO_TOP_EDGE_PIXEL_GAP: u32 = 72;
pub const EXPORTS_TO_RIGHT_EDGE_PIXEL_GAP: u32 = 120;
pub const IMPORTS_TO_TOP_EDGE_PIXEL_GAP: u32 = 72;
pub const IMPORTS_TO_LEFT_EDGE_PIXEL_GAP: u32 = 120;

// VIEWPORT
pub const VIEWPORT_ZOOM_WHEEL_RATE: f64 = (1. / 600.) * 3.;
pub const VIEWPORT_ZOOM_MOUSE_RATE: f64 = 1. / 400.;
pub const VIEWPORT_ZOOM_SCALE_MIN: f64 = 0.000_000_1;
pub const VIEWPORT_ZOOM_SCALE_MAX: f64 = 10_000.;
pub const VIEWPORT_ZOOM_MIN_FRACTION_COVER: f64 = 0.01;
pub const VIEWPORT_ZOOM_LEVELS: [f64; 74] = [
	0.0001, 0.000125, 0.00016, 0.0002, 0.00025, 0.00032, 0.0004, 0.0005, 0.00064, 0.0008, 0.001, 0.0016, 0.002, 0.0025, 0.0032, 0.004, 0.005, 0.0064, 0.008, 0.01, 0.01125, 0.015, 0.02, 0.025, 0.03,
	0.04, 0.05, 0.06, 0.08, 0.1, 0.125, 0.15, 0.2, 0.25, 0.33333333, 0.4, 0.5, 0.66666666, 0.8, 1., 1.25, 1.6, 2., 2.5, 3.2, 4., 5., 6.4, 8., 10., 12.5, 16., 20., 25., 32., 40., 50., 64., 80., 100.,
	128., 160., 200., 256., 320., 400., 512., 640., 800., 1024., 1280., 1600., 2048., 2560.,
];
/// Higher values create a steeper curve (a faster zoom rate change)
pub const VIEWPORT_ZOOM_WHEEL_RATE_CHANGE: f64 = 3.;

/// Helps push values that end in approximately half, plus or minus some floating point imprecision, towards the same side of the round() function.
pub const VIEWPORT_GRID_ROUNDING_BIAS: f64 = 0.002;

pub const VIEWPORT_SCROLL_RATE: f64 = 0.6;

pub const VIEWPORT_ROTATE_SNAP_INTERVAL: f64 = 15.;

pub const VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR: f64 = 0.95;

pub const DRAG_BEYOND_VIEWPORT_MAX_OVEREXTENSION_PIXELS: f64 = 50.;
pub const DRAG_BEYOND_VIEWPORT_SPEED_FACTOR: f64 = 20.;

// SNAPPING POINT
pub const SNAP_POINT_TOLERANCE: f64 = 5.;
/// These are layers whose bounding boxes are used for alignment.
pub const MAX_ALIGNMENT_CANDIDATES: usize = 100;
/// These are layers that are used for the layer snapper.
pub const MAX_SNAP_CANDIDATES: usize = 10;
/// These are points (anchors and bounding box corners etc.) in the layer snapper.
pub const MAX_LAYER_SNAP_POINTS: usize = 100;

pub const DRAG_THRESHOLD: f64 = 1.;

// TRANSFORMING LAYER
pub const ROTATE_INCREMENT: f64 = 15.;
pub const SCALE_INCREMENT: f64 = 0.1;
pub const SLOWING_DIVISOR: f64 = 10.;
pub const NUDGE_AMOUNT: f64 = 1.;
pub const BIG_NUDGE_AMOUNT: f64 = 10.;

// TOOLS
pub const DEFAULT_STROKE_WIDTH: f64 = 2.;

// SELECT TOOL
pub const SELECTION_TOLERANCE: f64 = 5.;
pub const DRAG_DIRECTION_MODE_DETERMINATION_THRESHOLD: f64 = 15.;
pub const SELECTION_DRAG_ANGLE: f64 = 90.;

// PIVOT
pub const PIVOT_CROSSHAIR_THICKNESS: f64 = 1.;
pub const PIVOT_CROSSHAIR_LENGTH: f64 = 9.;
pub const PIVOT_DIAMETER: f64 = 5.;
pub const DOWEL_PIN_RADIUS: f64 = 4.;

// COMPASS ROSE
pub const COMPASS_ROSE_RING_INNER_DIAMETER: f64 = 13.;
pub const COMPASS_ROSE_MAIN_RING_DIAMETER: f64 = 15.;
pub const COMPASS_ROSE_HOVER_RING_DIAMETER: f64 = 23.;
pub const COMPASS_ROSE_ARROW_SIZE: f64 = 5.;
// Angle to either side of the compass arrows where they are targetted by the cursor (in degrees, must be less than 45°)
pub const COMPASS_ROSE_ARROW_CLICK_TARGET_ANGLE: f64 = 20.;

// TRANSFORM OVERLAY
pub const ANGLE_MEASURE_RADIUS_FACTOR: f64 = 0.04;
pub const ARC_MEASURE_RADIUS_FACTOR_RANGE: (f64, f64) = (0.05, 0.15);

// TRANSFORM CAGE
pub const RESIZE_HANDLE_SIZE: f64 = 6.;
pub const BOUNDS_SELECT_THRESHOLD: f64 = 10.;
pub const BOUNDS_ROTATE_THRESHOLD: f64 = 20.;
pub const MIN_LENGTH_FOR_MIDPOINT_VISIBILITY: f64 = 20.;
pub const MIN_LENGTH_FOR_CORNERS_VISIBILITY: f64 = 12.;
/// The width or height that the transform cage needs to be (at least) before the corner resize handle click targets take up their full surroundings. Otherwise, when less than this value, the interior edge resize handle takes precedence so the corner handles don't eat into the edge area, making it harder to resize the cage from its edges.
pub const MIN_LENGTH_FOR_EDGE_RESIZE_PRIORITY_OVER_CORNERS: f64 = 10.;
/// When the width or height of the transform cage is less than this value, only the exterior of the bounding box will act as a click target for resizing.
pub const MIN_LENGTH_FOR_RESIZE_TO_INCLUDE_INTERIOR: f64 = 40.;
/// When dragging the edge of a cage with Alt, it centers around the pivot.
/// However if the pivot is on or near the same edge you are dragging, we should avoid scaling by a massive factor caused by the small denominator.
///
/// The motion of the user's cursor by an `x` pixel offset results in `x * scale_factor` pixels of offset on the other side.
pub const MAXIMUM_ALT_SCALE_FACTOR: f64 = 25.;
/// The width or height that the transform cage needs before it is considered to have no width or height.
pub const MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT: f64 = 1e-4;

// SKEW TRIANGLES
pub const SKEW_TRIANGLE_SIZE: f64 = 7.;
pub const SKEW_TRIANGLE_OFFSET: f64 = 4.;
pub const MIN_LENGTH_FOR_SKEW_TRIANGLE_VISIBILITY: f64 = 48.;

// PATH TOOL
pub const MANIPULATOR_GROUP_MARKER_SIZE: f64 = 6.;
pub const SELECTION_THRESHOLD: f64 = 10.;
pub const DRILL_THROUGH_THRESHOLD: f64 = 10.;
pub const HIDE_HANDLE_DISTANCE: f64 = 3.;
pub const HANDLE_ROTATE_SNAP_ANGLE: f64 = 15.;
pub const SEGMENT_INSERTION_DISTANCE: f64 = 5.;
pub const SEGMENT_OVERLAY_SIZE: f64 = 10.;
pub const SEGMENT_SELECTED_THICKNESS: f64 = 3.;
pub const HANDLE_LENGTH_FACTOR: f64 = 0.5;

// PEN TOOL
pub const CREATE_CURVE_THRESHOLD: f64 = 5.;

// SPLINE TOOL
pub const PATH_JOIN_THRESHOLD: f64 = 5.;

// LINE TOOL
pub const LINE_ROTATE_SNAP_ANGLE: f64 = 15.;

// BRUSH TOOL
pub const BRUSH_SIZE_CHANGE_KEYBOARD: f64 = 5.;
pub const DEFAULT_BRUSH_SIZE: f64 = 20.;

// GIZMOS
pub const POINT_RADIUS_HANDLE_SNAP_THRESHOLD: f64 = 8.;
pub const POINT_RADIUS_HANDLE_SEGMENT_THRESHOLD: f64 = 7.9;
pub const NUMBER_OF_POINTS_DIAL_SPOKE_EXTENSION: f64 = 1.2;
pub const NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH: f64 = 10.;
pub const ARC_SNAP_THRESHOLD: f64 = 5.;
pub const ARC_SWEEP_GIZMO_RADIUS: f64 = 14.;
pub const ARC_SWEEP_GIZMO_TEXT_HEIGHT: f64 = 12.;
pub const GIZMO_HIDE_THRESHOLD: f64 = 20.;
pub const GRID_ROW_COLUMN_GIZMO_OFFSET: f64 = 15.;

// SCROLLBARS
pub const SCROLLBAR_SPACING: f64 = 0.1;
pub const ASYMPTOTIC_EFFECT: f64 = 0.5;
pub const SCALE_EFFECT: f64 = 0.5;

// COLORS
pub const COLOR_OVERLAY_BLUE: &str = "#00a8ff";
pub const COLOR_OVERLAY_BLUE_50: &str = "#00a8ff80";
pub const COLOR_OVERLAY_YELLOW: &str = "#ffc848";
pub const COLOR_OVERLAY_YELLOW_DULL: &str = "#d7ba8b";
pub const COLOR_OVERLAY_GREEN: &str = "#63ce63";
pub const COLOR_OVERLAY_RED: &str = "#ef5454";
pub const COLOR_OVERLAY_GRAY: &str = "#cccccc";
pub const COLOR_OVERLAY_GRAY_25: &str = "#cccccc40";
pub const COLOR_OVERLAY_WHITE: &str = "#ffffff";
pub const COLOR_OVERLAY_BLACK_75: &str = "#000000bf";

// DOCUMENT
pub const FILE_EXTENSION: &str = "graphite";
pub const DEFAULT_DOCUMENT_NAME: &str = "Untitled Document";
pub const MAX_UNDO_HISTORY_LEN: usize = 100; // TODO: Add this to user preferences
pub const AUTO_SAVE_TIMEOUT_SECONDS: u64 = 1;

// INPUT
pub const DOUBLE_CLICK_MILLISECONDS: u64 = 500;
