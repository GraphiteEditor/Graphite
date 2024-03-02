// Viewport
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

pub const VIEWPORT_GRID_ROUNDING_BIAS: f64 = 0.002; // Helps push values that end in approximately half, plus or minus some floating point imprecision, towards the same side of the round() function

pub const VIEWPORT_SCROLL_RATE: f64 = 0.6;

pub const VIEWPORT_ROTATE_SNAP_INTERVAL: f64 = 15.;

pub const VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR: f64 = 0.95;

pub const DRAG_BEYOND_VIEWPORT_MAX_OVEREXTENSION_PIXELS: f64 = 50.;
pub const DRAG_BEYOND_VIEWPORT_SPEED_FACTOR: f64 = 0.5;

// Snapping point
pub const SNAP_POINT_TOLERANCE: f64 = 5.;

pub const DRAG_THRESHOLD: f64 = 1.;

// Transforming layer
pub const ROTATE_SNAP_ANGLE: f64 = 15.;
pub const SCALE_SNAP_INTERVAL: f64 = 0.1;
pub const SLOWING_DIVISOR: f64 = 10.;
pub const NUDGE_AMOUNT: f64 = 1.;
pub const BIG_NUDGE_AMOUNT: f64 = 10.;

// Select tool
pub const SELECTION_TOLERANCE: f64 = 5.;
pub const SELECTION_DRAG_ANGLE: f64 = 90.;
pub const PIVOT_CROSSHAIR_THICKNESS: f64 = 1.;
pub const PIVOT_CROSSHAIR_LENGTH: f64 = 9.;
pub const PIVOT_DIAMETER: f64 = 5.;

// Transformation cage
pub const BOUNDS_SELECT_THRESHOLD: f64 = 10.;
pub const BOUNDS_ROTATE_THRESHOLD: f64 = 20.;

// Path tool
pub const MANIPULATOR_GROUP_MARKER_SIZE: f64 = 6.;
pub const SELECTION_THRESHOLD: f64 = 10.;
pub const HIDE_HANDLE_DISTANCE: f64 = 3.;
pub const INSERT_POINT_ON_SEGMENT_TOO_FAR_DISTANCE: f64 = 50.;
pub const INSERT_POINT_ON_SEGMENT_TOO_CLOSE_DISTANCE: f64 = 5.;

// Pen tool
pub const CREATE_CURVE_THRESHOLD: f64 = 5.;

// Line tool
pub const LINE_ROTATE_SNAP_ANGLE: f64 = 15.;

// Brush tool
pub const BRUSH_SIZE_CHANGE_KEYBOARD: f64 = 5.;

// Scrollbars
pub const SCROLLBAR_SPACING: f64 = 0.1;
pub const ASYMPTOTIC_EFFECT: f64 = 0.5;
pub const SCALE_EFFECT: f64 = 0.5;

// Colors
pub const COLOR_OVERLAY_BLUE: &str = "#00a8ff";
pub const COLOR_OVERLAY_YELLOW: &str = "#ffc848";
pub const COLOR_OVERLAY_WHITE: &str = "#ffffff";
pub const COLOR_OVERLAY_GRAY: &str = "#cccccc";

// Fonts
pub const DEFAULT_FONT_FAMILY: &str = "Cabin";
pub const DEFAULT_FONT_STYLE: &str = "Normal (400)";

// Document
pub const DEFAULT_DOCUMENT_NAME: &str = "Untitled Document";
pub const FILE_SAVE_SUFFIX: &str = ".graphite";
pub const MAX_UNDO_HISTORY_LEN: usize = 100; // TODO: Add this to user preferences
pub const AUTO_SAVE_TIMEOUT_SECONDS: u64 = 15;
