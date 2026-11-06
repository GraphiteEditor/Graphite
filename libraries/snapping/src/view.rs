use crate::bounds::Bounds;
use glam::{DAffine2, DMat2, DMat3, DVec2, DVec3};

/// Maps points between a work plane and the screen, as a homography so it covers flat and perspective views alike.
///
/// Geometry is solved in the plane's coordinates, while nearness is judged in screen pixels through this mapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewMapping {
	plane_to_screen: DMat3,
	screen_to_plane: DMat3,
}

impl ViewMapping {
	pub const IDENTITY: Self = Self {
		plane_to_screen: DMat3::IDENTITY,
		screen_to_plane: DMat3::IDENTITY,
	};

	/// A flat view. Returns `None` if the transform collapses the plane onto a line or point.
	pub fn from_affine(plane_to_screen: DAffine2) -> Option<Self> {
		Self::from_homography(DMat3::from(plane_to_screen))
	}

	/// A view given as a homography that gives points in front of the viewer a positive weight. Returns `None` if it cannot be inverted.
	pub fn from_homography(plane_to_screen: DMat3) -> Option<Self> {
		let determinant = plane_to_screen.determinant();
		if !determinant.is_finite() || determinant == 0. {
			return None;
		}

		let screen_to_plane = plane_to_screen.inverse();
		screen_to_plane.is_finite().then_some(Self { plane_to_screen, screen_to_plane })
	}

	/// Where a plane point appears on screen. The result is not finite for a point at or beyond the horizon.
	pub fn to_screen(&self, plane_point: DVec2) -> DVec2 {
		project(self.plane_to_screen, plane_point)
	}

	/// The plane point under a screen point. The result is not finite for a screen point at or beyond the horizon.
	pub fn to_plane(&self, screen_point: DVec2) -> DVec2 {
		project(self.screen_to_plane, screen_point)
	}

	/// The distance in screen pixels between two plane points, which is infinite if either cannot be shown.
	pub fn screen_distance(&self, plane_a: DVec2, plane_b: DVec2) -> f64 {
		let distance = self.to_screen(plane_a).distance(self.to_screen(plane_b));
		if distance.is_finite() { distance } else { f64::INFINITY }
	}

	/// How a small step in the plane at a point becomes a step on screen, which is the same at every point of a flat view.
	/// It is zero for a point that cannot be shown.
	pub fn jacobian_at(&self, plane_point: DVec2) -> DMat2 {
		let matrix = self.plane_to_screen;
		let homogeneous = matrix * plane_point.extend(1.);
		let in_front = homogeneous.z > 0.;
		if !in_front {
			return DMat2::ZERO;
		}

		// Differentiating the division by the weight leaves the linear part less the screen position times the weight's own slope
		let screen = homogeneous.truncate() / homogeneous.z;
		let linear = DMat2::from_cols(matrix.x_axis.truncate(), matrix.y_axis.truncate());
		let weight_slope = DMat2::from_cols(screen * matrix.x_axis.z, screen * matrix.y_axis.z);
		(linear - weight_slope) * (1. / homogeneous.z)
	}

	/// A conservative box in the plane around everything that appears within `screen_radius` pixels of `plane_center`.
	/// It is empty when the center cannot be shown, and [`Bounds::EVERYTHING`] when that reach touches the horizon.
	pub fn plane_bounds_within_reach(&self, plane_center: DVec2, screen_radius: f64) -> Bounds {
		let screen_center = self.to_screen(plane_center);
		if !screen_center.is_finite() {
			return Bounds::EMPTY;
		}

		// The screen square around the reach maps to a convex quadrilateral in the plane, so its corners bound it
		let corners = [DVec2::new(-1., -1.), DVec2::new(1., -1.), DVec2::new(1., 1.), DVec2::new(-1., 1.)];

		let mut bounds = Bounds::EMPTY;
		let mut sign = 0.;
		for corner in corners {
			let homogeneous = self.screen_to_plane * (screen_center + corner * screen_radius).extend(1.);

			// The corners all lie on one side of the horizon exactly when their homogeneous weights share a sign
			if !homogeneous.is_finite() || homogeneous.z == 0. || homogeneous.z.signum() * sign < 0. {
				return Bounds::EVERYTHING;
			}
			sign = homogeneous.z.signum();

			bounds = bounds.including(homogeneous.truncate() / homogeneous.z);
		}

		if bounds.min.is_finite() && bounds.max.is_finite() { bounds } else { Bounds::EVERYTHING }
	}
}

fn project(matrix: DMat3, point: DVec2) -> DVec2 {
	let DVec3 { x, y, z } = matrix * point.extend(1.);

	// A weight of the wrong sign places the point behind the viewer, where the division would mirror it back into view
	if z > 0. { DVec2::new(x, y) / z } else { DVec2::NAN }
}

#[cfg(test)]
mod tests {
	use super::*;

	fn perspective() -> ViewMapping {
		// A floor seen from above its near edge, where plane points further along Y shrink toward a horizon
		ViewMapping::from_homography(DMat3::from_cols(DVec3::new(100., 0., 0.), DVec3::new(0., 100., 0.1), DVec3::new(400., 300., 1.))).unwrap()
	}

	#[test]
	fn flat_views_round_trip_and_scale_distances_by_the_zoom() {
		let view = ViewMapping::from_affine(DAffine2::from_scale_angle_translation(DVec2::splat(2.), 0.5, DVec2::new(30., -40.))).unwrap();
		let point = DVec2::new(12.5, -7.);

		assert!(view.to_plane(view.to_screen(point)).abs_diff_eq(point, 1e-9));
		assert!((view.screen_distance(DVec2::ZERO, DVec2::new(3., 4.)) - 10.).abs() < 1e-9);
	}

	#[test]
	fn collapsed_views_are_rejected() {
		assert!(ViewMapping::from_affine(DAffine2::from_scale(DVec2::new(1., 0.))).is_none());
		assert!(ViewMapping::from_homography(DMat3::ZERO).is_none());
	}

	#[test]
	fn perspective_views_round_trip_and_foreshorten() {
		let view = perspective();
		let point = DVec2::new(1.5, 2.);

		assert!(view.to_plane(view.to_screen(point)).abs_diff_eq(point, 1e-9));

		// Equal steps in the plane cover fewer pixels the further away they are
		let near = view.screen_distance(DVec2::new(0., 0.), DVec2::new(1., 0.));
		let far = view.screen_distance(DVec2::new(0., 5.), DVec2::new(1., 5.));
		assert!(far < near);
	}

	#[test]
	fn points_behind_the_viewer_are_not_shown() {
		let view = perspective();

		assert!(!view.to_screen(DVec2::new(0., -20.)).is_finite());
		assert_eq!(view.screen_distance(DVec2::ZERO, DVec2::new(0., -20.)), f64::INFINITY);
	}

	#[test]
	fn reach_bounds_contain_everything_within_reach() {
		let radius = 8.;
		let center = DVec2::new(1.5, 2.);

		for view in [
			ViewMapping::from_affine(DAffine2::from_scale_angle_translation(DVec2::new(3., 3.), 0.7, DVec2::new(5., 9.))).unwrap(),
			perspective(),
		] {
			let bounds = view.plane_bounds_within_reach(center, radius);
			let screen_center = view.to_screen(center);

			for step in 0..64 {
				let angle = step as f64 / 64. * std::f64::consts::TAU;
				let on_disc_edge = view.to_plane(screen_center + DVec2::from_angle(angle) * radius);
				assert!(bounds.inflated(1e-9).contains(on_disc_edge));
			}
		}
	}

	#[test]
	fn reach_bounds_give_up_at_the_horizon() {
		let view = perspective();

		// This view's horizon is the screen line y = 1000, which points approach as their plane Y grows
		let far_away = DVec2::new(0., 1e9);
		assert_eq!(view.plane_bounds_within_reach(far_away, 8.), Bounds::EVERYTHING);

		// Nothing can be near a point behind the viewer
		assert_eq!(view.plane_bounds_within_reach(DVec2::new(0., -20.), 8.), Bounds::EMPTY);
	}

	#[test]
	fn the_jacobian_predicts_how_small_steps_appear_on_screen() {
		let point = DVec2::new(1.5, 2.);
		let step = DVec2::new(3e-6, -2e-6);

		for view in [
			ViewMapping::from_affine(DAffine2::from_scale_angle_translation(DVec2::new(3., 0.5), 0.7, DVec2::new(5., 9.))).unwrap(),
			perspective(),
		] {
			let predicted = view.jacobian_at(point) * step;
			let actual = view.to_screen(point + step) - view.to_screen(point);
			assert!(predicted.abs_diff_eq(actual, 1e-9));
		}

		assert_eq!(perspective().jacobian_at(DVec2::new(0., -20.)), DMat2::ZERO);
	}
}
