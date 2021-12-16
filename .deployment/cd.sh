#!/bin/sh

git switch master || git switch -c master

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
cd frontend
npm ci

echo 📦 Install cargo-about
cargo install cargo-about

echo 👷 Build Graphite web client
npm run build
