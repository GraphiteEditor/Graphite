#!/bin/sh

# Switch to the correct branch
if [[ -z "${CF_PAGES_BRANCH}" ]]; then
	git switch master || git switch -c unknown-branch
else
	git switch $CF_PAGES_BRANCH || git switch -c $CF_PAGES_BRANCH
fi

# Install the latest version of the Rust toolchain
echo 🔧 Install Rust
curl https://sh.rustup.rs -sSf | sh -s -- -y
export PATH=$PATH:/opt/buildhome/.cargo/bin
echo rustc version:
rustc --version

# Install the project's Node dependencies through npm
echo 🚧 Install Node dependencies
echo node version:
node --version
echo npm version:
npm --version
cd frontend
npm ci

# Install the cargo-about Rust dependency that's used during the Webpack build process (in `webpack.config.js`)
echo 📦 Install cargo-about
cargo install cargo-about

# Build for production
echo 👷 Build Graphite web client
export NODE_ENV=production
npm run build && mv public dist # `&&` is used here to preserve the exit code
