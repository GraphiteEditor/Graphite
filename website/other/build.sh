#!/bin/sh
set -e # Exit with nonzero exit code if any individual command fails throughout the script

echo 📁 Create output directory in 'website/other/dist'
cd website/other
mkdir dist

echo 🔧 Install the latest Rust
curl https://sh.rustup.rs -sSf | sh -s -- -y
export PATH=$PATH:/opt/buildhome/.cargo/bin
rustup update stable
echo rustc version:
rustc --version

echo 📦 Install wasm-pack
cargo install wasm-pack
echo wasm-pack version:
wasm-pack --version

echo 🚧 Print installed node and npm versions
echo node version:
node --version
echo npm version:
npm --version

echo 👷 Build Bezier-rs demos to 'website/other/dist/libraries/bezier-rs'
mkdir dist/libraries
mkdir dist/libraries/bezier-rs
cd bezier-rs-demos
npm ci
NODE_ENV=production npm run build
cp ../../static/fonts/common.css dist/fonts.css
mv dist/* ../dist/libraries/bezier-rs
cd ..
