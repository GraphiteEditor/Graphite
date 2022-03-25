Data can **flow** or **composite**.

# Types
Data types in Rust.

- Color - A color description freely convertible between color models and color spaces.
- Raster - A monadic data format that allows a color to be sampled at any rectangle position.
<!-- - CSG - Constructive Solid Geometry. What's inside and outside of a shape. -->
<!-- - SDF - Signed Distance Field. Distance from the surface of a CSG. -->

# Table Archetypes
A set of standard named columns in a table of specific type(s), where some can be optional.

- XY - A vector2 (2D vector).  
 `X`: number, `Y`: number

- XYZ - A vector3 (3D vector).  
  `X`: number, `Y`: number, `Z`: number

- XYZW - A vector4 (4D vector).  
  `X`: number, `Y`: number, `Z`: number, `W`: number

- Trace - Sequence of points in 2D or (rarely) 3D space. Often recorded as mouse or stylus movements where time is seconds since the stroke began.  
  `X`: number, `Y`: number, `Z?`: number, `Pressure?`: number, `Pitch?`: number, `Roll?`: number, `Yaw?`: number, `Time?`: number
