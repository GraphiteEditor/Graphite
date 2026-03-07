use std::ops::{Add, Div, Mul, Sub};

use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::DVec2;

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type, ExtractField)]
pub struct ViewportMessageHandler {
	bounds: Bounds,
	// Ratio of logical pixels to physical pixels
	scale: f64,
}
impl Default for ViewportMessageHandler {
	fn default() -> Self {
		Self {
			bounds: Bounds {
				offset: Point { x: 0., y: 0. },
				size: Point { x: 0., y: 0. },
			},
			scale: 1.0,
		}
	}
}

#[message_handler_data]
impl MessageHandler<ViewportMessage, ()> for ViewportMessageHandler {
	fn process_message(&mut self, message: ViewportMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			ViewportMessage::Update { x, y, width, height, scale } => {
				assert!(scale > 0., "Viewport scale must be greater than zero");
				self.scale = scale;

				self.bounds = Bounds {
					offset: Point { x, y },
					size: Point { x: width, y: height },
				};
				responses.add(NodeGraphMessage::UpdateNodeGraphWidth);
			}
			ViewportMessage::RepropagateUpdate => {}
		}

		#[cfg(not(target_family = "wasm"))]
		{
			let physical_bounds = self.bounds().to_physical();
			responses.add(FrontendMessage::UpdateViewportPhysicalBounds {
				x: physical_bounds.x(),
				y: physical_bounds.y(),
				width: physical_bounds.width(),
				height: physical_bounds.height(),
			});
		}

		responses.add(NavigationMessage::CanvasPan { delta: DVec2::ZERO });

		if self.is_valid() {
			responses.add(DeferMessage::AfterGraphRun {
				messages: vec![
					DeferMessage::AfterGraphRun {
						messages: vec![DeferMessage::TriggerNavigationReady.into()],
					}
					.into(),
				],
			});
		}
	}

	advertise_actions!(ViewportMessageDiscriminant;);
}

impl ViewportMessageHandler {
	pub fn scale(&self) -> f64 {
		self.scale
	}

	pub fn bounds(&self) -> LogicalBounds {
		self.bounds.into_scaled(self.scale)
	}

	pub fn offset(&self) -> LogicalPoint {
		self.bounds.offset.into_scaled(self.scale)
	}

	pub fn size(&self) -> LogicalPoint {
		self.bounds.size().into_scaled(self.scale)
	}

	#[expect(private_bounds)]
	pub fn logical<T: Into<Point>>(&self, point: T) -> LogicalPoint {
		point.into().convert_to_logical(self.scale)
	}

	#[expect(private_bounds)]
	pub fn physical<T: Into<Point>>(&self, point: T) -> PhysicalPoint {
		point.into().convert_to_physical(self.scale)
	}

	pub fn center_in_viewport_space(&self) -> LogicalPoint {
		let size = self.size();
		LogicalPoint {
			inner: Point { x: size.x() / 2., y: size.y() / 2. },
			scale: size.scale,
		}
	}

	pub fn center_in_window_space(&self) -> LogicalPoint {
		let size = self.size();
		let offset = self.offset();
		LogicalPoint {
			inner: Point {
				x: (size.x() / 2.) + offset.x(),
				y: (size.y() / 2.) + offset.y(),
			},
			scale: size.scale,
		}
	}

	pub fn is_valid(&self) -> bool {
		self.scale > 0. && self.bounds.size.x() > 0. && self.bounds.size.y() > 0. && self.bounds.offset.x() >= 0. && self.bounds.offset.y() >= 0.
	}

	pub(crate) fn is_in_bounds(&self, point: LogicalPoint) -> bool {
		point.x() >= self.bounds.x() && point.y() >= self.bounds.y() && point.x() <= self.bounds.x() + self.bounds.width() && point.y() <= self.bounds.y() + self.bounds.height()
	}
}

pub trait ToLogical<L: ToPhysical<Self> + ?Sized> {
	fn to_logical(self) -> L;
}
pub trait ToPhysical<P: ToLogical<Self> + ?Sized> {
	fn to_physical(self) -> P;
}

trait IntoScaled<T: Scaled>: Sized {
	fn into_scaled(self, scale: f64) -> T;
}
trait FromWithScale<T>: Sized {
	fn from_with_scale(value: T, scale: f64) -> Self;
}
impl<T, U: Scaled> IntoScaled<U> for T
where
	U: FromWithScale<T>,
{
	fn into_scaled(self, scale: f64) -> U {
		U::from_with_scale(self, scale)
	}
}

trait AsPoint {
	fn as_point(&self) -> Point;
}

trait Scaled {
	fn scale(&self) -> f64;
}

pub trait Position {
	fn x(&self) -> f64;
	fn y(&self) -> f64;
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
struct Point {
	x: f64,
	y: f64,
}
impl Point {
	fn convert_to_logical(&self, scale: f64) -> LogicalPoint {
		Point { x: self.x(), y: self.y() }.into_scaled(scale)
	}
	fn convert_to_physical(&self, scale: f64) -> PhysicalPoint {
		Point {
			x: self.x() / scale,
			y: self.y() / scale,
		}
		.into_scaled(scale)
	}
}
impl Position for Point {
	fn x(&self) -> f64 {
		self.x
	}
	fn y(&self) -> f64 {
		self.y
	}
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct LogicalPoint {
	inner: Point,
	scale: f64,
}
impl AsPoint for LogicalPoint {
	fn as_point(&self) -> Point {
		self.inner
	}
}
impl Scaled for LogicalPoint {
	fn scale(&self) -> f64 {
		self.scale
	}
}
impl Position for LogicalPoint {
	fn x(&self) -> f64 {
		self.inner.x()
	}
	fn y(&self) -> f64 {
		self.inner.y()
	}
}
impl ToPhysical<PhysicalPoint> for LogicalPoint {
	fn to_physical(self) -> PhysicalPoint {
		PhysicalPoint { inner: self.inner, scale: self.scale }
	}
}
impl FromWithScale<Point> for LogicalPoint {
	fn from_with_scale(value: Point, scale: f64) -> Self {
		Self { inner: value, scale }
	}
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PhysicalPoint {
	inner: Point,
	scale: f64,
}
impl AsPoint for PhysicalPoint {
	fn as_point(&self) -> Point {
		self.inner
	}
}
impl Scaled for PhysicalPoint {
	fn scale(&self) -> f64 {
		self.scale
	}
}
impl Position for PhysicalPoint {
	fn x(&self) -> f64 {
		self.inner.x() * self.scale
	}
	fn y(&self) -> f64 {
		self.inner.y() * self.scale
	}
}
impl ToLogical<LogicalPoint> for PhysicalPoint {
	fn to_logical(self) -> LogicalPoint {
		LogicalPoint { inner: self.inner, scale: self.scale }
	}
}
impl FromWithScale<Point> for PhysicalPoint {
	fn from_with_scale(value: Point, scale: f64) -> Self {
		Self { inner: value, scale }
	}
}

pub trait Rect<P: Position>: Position {
	fn offset(&self) -> P;
	fn size(&self) -> P;
	fn width(&self) -> f64;
	fn height(&self) -> f64;
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type, ExtractField)]
struct Bounds {
	offset: Point,
	size: Point,
}
impl Position for Bounds {
	fn x(&self) -> f64 {
		self.offset.x()
	}
	fn y(&self) -> f64 {
		self.offset.y()
	}
}
impl Rect<Point> for Bounds {
	fn offset(&self) -> Point {
		self.offset
	}
	fn size(&self) -> Point {
		self.size
	}
	fn width(&self) -> f64 {
		self.size.x()
	}
	fn height(&self) -> f64 {
		self.size.y()
	}
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct LogicalBounds {
	offset: Point,
	size: Point,
	scale: f64,
}
impl Scaled for LogicalBounds {
	fn scale(&self) -> f64 {
		self.scale
	}
}
impl Position for LogicalBounds {
	fn x(&self) -> f64 {
		self.offset.x()
	}
	fn y(&self) -> f64 {
		self.offset.y()
	}
}
impl Rect<LogicalPoint> for LogicalBounds {
	fn offset(&self) -> LogicalPoint {
		self.offset.into_scaled(self.scale)
	}
	fn size(&self) -> LogicalPoint {
		self.size.into_scaled(self.scale)
	}
	fn width(&self) -> f64 {
		self.size.x()
	}
	fn height(&self) -> f64 {
		self.size.y()
	}
}
impl ToPhysical<PhysicalBounds> for LogicalBounds {
	fn to_physical(self) -> PhysicalBounds {
		PhysicalBounds {
			offset: self.offset,
			size: self.size,
			scale: self.scale,
		}
	}
}
impl FromWithScale<Bounds> for LogicalBounds {
	fn from_with_scale(value: Bounds, scale: f64) -> Self {
		Self {
			offset: value.offset(),
			size: value.size(),
			scale,
		}
	}
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PhysicalBounds {
	offset: Point,
	size: Point,
	scale: f64,
}
impl Scaled for PhysicalBounds {
	fn scale(&self) -> f64 {
		self.scale
	}
}
impl Position for PhysicalBounds {
	fn x(&self) -> f64 {
		self.offset.x() * self.scale
	}
	fn y(&self) -> f64 {
		self.offset.y() * self.scale
	}
}
impl Rect<PhysicalPoint> for PhysicalBounds {
	fn offset(&self) -> PhysicalPoint {
		self.offset.into_scaled(self.scale)
	}
	fn size(&self) -> PhysicalPoint {
		self.size.into_scaled(self.scale)
	}
	fn width(&self) -> f64 {
		self.size.x() * self.scale
	}
	fn height(&self) -> f64 {
		self.size.y() * self.scale
	}
}
impl ToLogical<LogicalBounds> for PhysicalBounds {
	fn to_logical(self) -> LogicalBounds {
		LogicalBounds {
			offset: self.offset,
			size: self.size,
			scale: self.scale,
		}
	}
}
impl FromWithScale<Bounds> for PhysicalBounds {
	fn from_with_scale(value: Bounds, scale: f64) -> Self {
		Self {
			offset: value.offset(),
			size: value.size(),
			scale,
		}
	}
}

impl Mul<f64> for Point {
	type Output = Point;
	fn mul(self, rhs: f64) -> Self::Output {
		assert_ne!(rhs, 0.0, "Cannot multiply point by zero");
		Point { x: self.x * rhs, y: self.y * rhs }
	}
}
impl Div<f64> for Point {
	type Output = Point;
	fn div(self, rhs: f64) -> Self::Output {
		assert_ne!(rhs, 0.0, "Cannot divide point by zero");
		Point { x: self.x / rhs, y: self.y / rhs }
	}
}
impl Add<f64> for Point {
	type Output = Point;
	fn add(self, rhs: f64) -> Self::Output {
		Point { x: self.x + rhs, y: self.y + rhs }
	}
}
impl Sub<f64> for Point {
	type Output = Point;
	fn sub(self, rhs: f64) -> Self::Output {
		Point { x: self.x - rhs, y: self.y - rhs }
	}
}
impl Mul<Point> for Point {
	type Output = Point;
	fn mul(self, rhs: Point) -> Self::Output {
		assert_ne!(rhs.x, 0.0, "Cannot multiply point by zero");
		assert_ne!(rhs.y, 0.0, "Cannot multiply point by zero");
		Point { x: self.x * rhs.x, y: self.y * rhs.y }
	}
}
impl Div<Point> for Point {
	type Output = Point;
	fn div(self, rhs: Point) -> Self::Output {
		assert_ne!(rhs.x, 0.0, "Cannot multiply point by zero");
		assert_ne!(rhs.y, 0.0, "Cannot multiply point by zero");
		Point { x: self.x / rhs.x, y: self.y / rhs.y }
	}
}
impl Add<Point> for Point {
	type Output = Point;
	fn add(self, rhs: Point) -> Self::Output {
		Point { x: self.x + rhs.x, y: self.y + rhs.y }
	}
}
impl Sub<Point> for Point {
	type Output = Point;
	fn sub(self, rhs: Point) -> Self::Output {
		Point { x: self.x - rhs.x, y: self.y - rhs.y }
	}
}

impl Mul<f64> for Bounds {
	type Output = Bounds;
	fn mul(self, rhs: f64) -> Self::Output {
		assert_ne!(rhs, 0.0, "Cannot multiply bounds by zero");
		Bounds {
			offset: self.offset * rhs,
			size: self.size * rhs,
		}
	}
}
impl Div<f64> for Bounds {
	type Output = Bounds;
	fn div(self, rhs: f64) -> Self::Output {
		assert_ne!(rhs, 0.0, "Cannot divide bounds by zero");
		Bounds {
			offset: self.offset / rhs,
			size: self.size / rhs,
		}
	}
}

impl Mul<LogicalPoint> for LogicalPoint {
	type Output = LogicalPoint;
	fn mul(self, rhs: LogicalPoint) -> Self::Output {
		assert_scale(&self, &rhs);
		(self.as_point() * rhs.as_point()).into_scaled(self.scale())
	}
}
impl Div<LogicalPoint> for LogicalPoint {
	type Output = LogicalPoint;
	fn div(self, rhs: LogicalPoint) -> Self::Output {
		assert_scale(&self, &rhs);
		(self.as_point() / rhs.as_point()).into_scaled(self.scale())
	}
}
impl Add<LogicalPoint> for LogicalPoint {
	type Output = LogicalPoint;
	fn add(self, rhs: LogicalPoint) -> Self::Output {
		assert_scale(&self, &rhs);
		(self.as_point() + rhs.as_point()).into_scaled(self.scale())
	}
}
impl Sub<LogicalPoint> for LogicalPoint {
	type Output = LogicalPoint;
	fn sub(self, rhs: LogicalPoint) -> Self::Output {
		assert_scale(&self, &rhs);
		(self.as_point() - rhs.as_point()).into_scaled(self.scale())
	}
}
impl Mul<PhysicalPoint> for PhysicalPoint {
	type Output = PhysicalPoint;
	fn mul(self, rhs: PhysicalPoint) -> Self::Output {
		assert_scale(&self, &rhs);
		(self.as_point() * rhs.as_point()).into_scaled(self.scale())
	}
}
impl Div<PhysicalPoint> for PhysicalPoint {
	type Output = PhysicalPoint;
	fn div(self, rhs: PhysicalPoint) -> Self::Output {
		assert_scale(&self, &rhs);
		(self.as_point() / rhs.as_point()).into_scaled(self.scale())
	}
}
impl Add<PhysicalPoint> for PhysicalPoint {
	type Output = PhysicalPoint;
	fn add(self, rhs: PhysicalPoint) -> Self::Output {
		assert_scale(&self, &rhs);
		(self.as_point() + rhs.as_point()).into_scaled(self.scale())
	}
}
impl Sub<PhysicalPoint> for PhysicalPoint {
	type Output = PhysicalPoint;
	fn sub(self, rhs: PhysicalPoint) -> Self::Output {
		assert_scale(&self, &rhs);
		(self.as_point() - rhs.as_point()).into_scaled(self.scale())
	}
}
fn assert_scale<T: Scaled>(a: &T, b: &T) {
	assert_eq!(a.scale(), b.scale(), "Cannot multiply with diffent scale");
}

impl From<(f64, f64)> for Point {
	fn from((x, y): (f64, f64)) -> Self {
		Self { x, y }
	}
}
impl From<(f64, f64, f64, f64)> for Bounds {
	fn from((x, y, width, height): (f64, f64, f64, f64)) -> Self {
		Self {
			offset: Point { x, y },
			size: Point { x: width, y: height },
		}
	}
}

impl From<LogicalPoint> for (f64, f64) {
	fn from(point: LogicalPoint) -> Self {
		(point.x(), point.y())
	}
}
impl From<PhysicalPoint> for (f64, f64) {
	fn from(point: PhysicalPoint) -> Self {
		(point.x(), point.y())
	}
}
impl From<LogicalBounds> for (f64, f64, f64, f64) {
	fn from(bounds: LogicalBounds) -> Self {
		(bounds.x(), bounds.y(), bounds.width(), bounds.height())
	}
}
impl From<PhysicalBounds> for (f64, f64, f64, f64) {
	fn from(bounds: PhysicalBounds) -> Self {
		(bounds.x(), bounds.y(), bounds.width(), bounds.height())
	}
}

impl From<glam::DVec2> for Point {
	fn from(vec: glam::DVec2) -> Self {
		Self { x: vec.x, y: vec.y }
	}
}
impl From<LogicalPoint> for glam::DVec2 {
	fn from(val: LogicalPoint) -> Self {
		glam::DVec2::new(val.x(), val.y())
	}
}
impl From<PhysicalPoint> for glam::DVec2 {
	fn from(val: PhysicalPoint) -> Self {
		glam::DVec2::new(val.x(), val.y())
	}
}

impl From<[glam::DVec2; 2]> for Bounds {
	fn from(bounds: [glam::DVec2; 2]) -> Self {
		Self {
			offset: bounds[0].into(),
			size: Point {
				x: bounds[1].x - bounds[0].x,
				y: bounds[1].y - bounds[0].y,
			},
		}
	}
}
impl From<LogicalBounds> for [glam::DVec2; 2] {
	fn from(bounds: LogicalBounds) -> Self {
		[glam::DVec2::new(bounds.x(), bounds.y()), glam::DVec2::new(bounds.x() + bounds.width(), bounds.y() + bounds.height())]
	}
}
impl From<PhysicalBounds> for [glam::DVec2; 2] {
	fn from(bounds: PhysicalBounds) -> Self {
		[glam::DVec2::new(bounds.x(), bounds.y()), glam::DVec2::new(bounds.x() + bounds.width(), bounds.y() + bounds.height())]
	}
}

impl LogicalPoint {
	pub fn into_dvec2(self) -> DVec2 {
		DVec2::new(self.x(), self.y())
	}
}
impl PhysicalPoint {
	pub fn into_dvec2(self) -> DVec2 {
		DVec2::new(self.x(), self.y())
	}
}
