#!/bin/sh

echo 🔧 Install Rust
curl https://sh.rustup.rs -sSf | sh -s -- -y
export PATH=$PATH:/opt/buildhome/.cargo/bin
rust --version

echo 🚧 Install Node dependencies
export NODE_VERSION=16
node --version
npm --version
cd frontend
npm install

echo 📦 Install cargo-about
cargo install cargo-about

echo 👷 Build Graphite web client
npm run build
