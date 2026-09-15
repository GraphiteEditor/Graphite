/// Minimum allowable separation between adjacent `t` values when calculating curve intersections
pub(crate) const MIN_SEPARATION_VALUE: f64 = 5. * 1e-3;

/// Threshold for comparing floating point values in intersection and centroid math.
pub(crate) const MAX_ABSOLUTE_DIFFERENCE: f64 = 1e-3;

/// Maximum distance at which two points are treated as one and the same point.
pub(crate) const MAX_COINCIDENT_POINT_DISTANCE: f64 = 1e-7;
