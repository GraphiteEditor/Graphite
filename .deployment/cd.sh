#!/bin/sh

echo 🔧 Install Rust
curl https://sh.rustup.rs -sSf | sh -s -- -y
export PATH=$PATH:/opt/buildhome/.cargo/bin

echo 🚧 Install Node dependencies
cd frontend
npm install

echo 📦 Install cargo-about
cargo install cargo-about

echo 👷 Build Graphite web client
npm run build
