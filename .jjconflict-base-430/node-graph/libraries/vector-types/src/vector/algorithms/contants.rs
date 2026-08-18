/// Minimum allowable separation between adjacent `t` values when calculating curve intersections
pub(crate) const MIN_SEPARATION_VALUE: f64 = 5. * 1e-3;

/// Constant used to determine if `f64`s are equivalent.
#[cfg(test)]
pub(crate) const MAX_ABSOLUTE_DIFFERENCE: f64 = 1e-3;
