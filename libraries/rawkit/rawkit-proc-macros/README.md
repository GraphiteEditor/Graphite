# Rawkit-proc-macros

Procedural macros for Rawkit.

This library is intended to be used by Rawkit. You should not be depending on this crate directly.

### Tag

A derive macro that helps to specify which metadata needs to be extracted from IFD.

### build_camera_data

A procedural macro that reads the data of all cameras from the toml files and returns the bundled data. Helps to include camera data as part of binary.
