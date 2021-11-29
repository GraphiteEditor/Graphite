#!/bin/sh

echo 🔧 Install Rust
curl https://sh.rustup.rs -sSf | sh -s -- -y
export PATH=$PATH:/opt/buildhome/.cargo/bin
echo Rust version:
/opt/buildhome/.cargo/bin/rust --version

echo 🚧 Install Node dependencies
nvm use 16
echo node version:
node --version
echo npm version:
npm --version
cd frontend
npm install

echo 📦 Install cargo-about
cargo install cargo-about

echo 👷 Build Graphite web client
npm run build
