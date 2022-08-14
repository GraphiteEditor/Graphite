#!/bin/sh

echo 🔧 Install Rust
curl https://sh.rustup.rs -sSf | sh -s -- -y
export PATH=$PATH:/opt/buildhome/.cargo/bin
echo rustc version:
rustc --version

echo 🚧 Install Node dependencies
echo node version:
node --version
echo npm version:
npm --version
cd bezier-rs-demos
npm ci

echo 👷 Build Bezier-rs Demos
export NODE_ENV=production
npm run build
mv dist ../../public/bezier-rs-demos
