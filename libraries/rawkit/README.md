[crates.io](https://crates.io/crates/rawkit) • [docs.rs](https://docs.rs/rawkit) • [repo](https://github.com/GraphiteEditor/Graphite/tree/master/libraries/rawkit)

# Rawkit

A library to extract images from camera raw files.

**WARNING**: This library is currently in-progress and not functional yet.

This library is built to extract the images from the raw files of Sony's cameras. In the future the library will add support for all other major camera manufacturers.

Rawkit is built for the needs of [Graphite](https://graphite.rs), an open source 2D graphics editor. We hope it may be useful to others, but presently Graphite is its primary user. Pull requests are welcomed for new features, code cleanup, ergonomic enhancements, performance improvements, and documentation clarifications.

### Using Rawkit

```rust
use rawkit::RawImage;
use rawkit::tiff::values::Transform;

// Open a file for reading
let file = BufReader::new(File::open("example.arw")?);

// Decode the file to extract the raw pixels and its associated metadata
let mut raw_image = RawImage::decode(file);

// All the raw pixel data and metadata is stored within raw_image
println!("Initial Bayer pixel values: {:?}", raw_image.data[:10]);
println!("Image size: {} x {}", raw_image.width, raw_image.height);
println!("CFA Pattern: {:?}", raw_image.cfa_pattern);
println!("Camera Model: {:?}", raw_image.camera_model);
println!("White balance: {:?}", raw_image.white_balance);

// The metadata could also be edited if the extracted metadata needs to be customized
raw_image.white_balance = Some([2609, 1024, 1024, 1220]); // For RGGB camera
raw_image.transform = Transform::Rotate90;

// Process the raw image into a RGB image
let image = raw_image.process_8bit();

// The final image will be stored within image
println!("Initial RGB pixel values: {:?}", image.data[:10]);
println!("Image size: {} x {}", image.width, image.height);
```
