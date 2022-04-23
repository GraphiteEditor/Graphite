use graphene::color::Color;

// Viewport
pub const VIEWPORT_ZOOM_WHEEL_RATE: f64 = 1. / 600.;
pub const VIEWPORT_ZOOM_MOUSE_RATE: f64 = 1. / 400.;
pub const VIEWPORT_ZOOM_SCALE_MIN: f64 = 0.000_000_1;
pub const VIEWPORT_ZOOM_SCALE_MAX: f64 = 10_000.;
pub const VIEWPORT_ZOOM_LEVELS: [f64; 74] = [
	0.0001, 0.000125, 0.00016, 0.0002, 0.00025, 0.00032, 0.0004, 0.0005, 0.00064, 0.0008, 0.001, 0.0016, 0.002, 0.0025, 0.0032, 0.004, 0.005, 0.0064, 0.008, 0.01, 0.01125, 0.015, 0.02, 0.025, 0.03,
	0.04, 0.05, 0.06, 0.08, 0.1, 0.125, 0.15, 0.2, 0.25, 0.33333333, 0.4, 0.5, 0.66666666, 0.8, 1., 1.25, 1.6, 2., 2.5, 3.2, 4., 5., 6.4, 8., 10., 12.5, 16., 20., 25., 32., 40., 50., 64., 80., 100.,
	128., 160., 200., 256., 320., 400., 512., 640., 800., 1024., 1280., 1600., 2048., 2560.,
];

pub const VIEWPORT_SCROLL_RATE: f64 = 0.6;

pub const VIEWPORT_ROTATE_SNAP_INTERVAL: f64 = 15.;

pub const SNAP_TOLERANCE: f64 = 3.;
pub const SNAP_OVERLAY_FADE_DISTANCE: f64 = 20.;
pub const SNAP_OVERLAY_UNSNAPPED_OPACITY: f64 = 0.4;

pub const DRAG_THRESHOLD: f64 = 1.;

// Transforming layer
pub const ROTATE_SNAP_ANGLE: f64 = 15.;
pub const SCALE_SNAP_INTERVAL: f64 = 0.1;
pub const SLOWING_DIVISOR: f64 = 10.;

// Select tool
pub const SELECTION_TOLERANCE: f64 = 1.;
pub const SELECTION_DRAG_ANGLE: f64 = 90.;

// Transformation cage
pub const BOUNDS_SELECT_THRESHOLD: f64 = 10.;
pub const BOUNDS_ROTATE_THRESHOLD: f64 = 20.;

// Path tool
pub const VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE: f64 = 5.;
pub const SELECTION_THRESHOLD: f64 = 10.;

// Pen tool
pub const CREATE_CURVE_THRESHOLD: f64 = 5.;

// Line tool
pub const LINE_ROTATE_SNAP_ANGLE: f64 = 15.;

// Scrollbars
pub const SCROLLBAR_SPACING: f64 = 0.1;
pub const ASYMPTOTIC_EFFECT: f64 = 0.5;
pub const SCALE_EFFECT: f64 = 0.5;

pub const DEFAULT_DOCUMENT_NAME: &str = "Untitled Document";
pub const FILE_SAVE_SUFFIX: &str = ".graphite";
pub const FILE_EXPORT_SUFFIX: &str = ".svg";

// Colors
pub const COLOR_ACCENT: Color = Color::from_unsafe(0x00 as f32 / 255., 0xA8 as f32 / 255., 0xFF as f32 / 255.);

// Document
pub const GRAPHITE_DOCUMENT_VERSION: &str = "0.0.7";
pub const VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR: f32 = 1.05;
