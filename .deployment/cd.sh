#!/bin/sh

if [[ -z "${CF_PAGES_BRANCH}" ]]; then
	git switch master || git switch -c unknown-branch
else
	git switch $CF_PAGES_BRANCH || git switch -c $CF_PAGES_BRANCH
fi

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
export NODE_ENV=production
npm run build
