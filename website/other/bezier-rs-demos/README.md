# Bezier-rs interactive documentation

Open these interactive docs: <https://graphite.rs/libraries/bezier-rs/>

This page also serves isolated demos for iframes used in the Rustdoc [crate documentation](https://docs.rs/bezier-rs/latest/bezier_rs/).

## Building and running

From this directory, first execute `npm install` to install the required Node dependencies. Then...

- To run the development server with hot reloading:
  ```
  npm start
  ```
- To compile an unoptimized development build (like above, but it writes the files instead of serving them):
  ```
  npm run build
  ```
- To compile an optimized production build:
  
  ```
  # WSL/Mac/Linux terminals:
  npm run build-prod-unix

  # Windows terminals:
  npm run build-prod-windows
  ```

When a build is compiled, the entire `./public` folder is the output containing both the static `index.html`, etc., plus the generated `build/` folder.
