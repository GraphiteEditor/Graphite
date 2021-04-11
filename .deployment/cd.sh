#!/bin/sh

echo 🔧 Install Rust
curl https://sh.rustup.rs -sSf | sh -s -- -y
export PATH=$PATH:/opt/buildhome/.cargo/bin

echo 🚧 Install Node dependencies
cd client/web
npm install

echo 👷 Build Graphite web client
npm run build
