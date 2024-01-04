#!/bin/sh

echo 📁 Create output directory in 'website/other/dist'
cd website/other
mkdir dist

echo 🔧 Install the latest Rust
curl https://sh.rustup.rs -sSf | sh -s -- -y
export PATH=$PATH:/opt/buildhome/.cargo/bin
rustup update stable
echo rustc version:
rustc --version

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
mv dist/* ../dist/libraries/bezier-rs
cd ..
