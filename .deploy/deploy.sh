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

# Install the cargo-about Rust dependency that's used during the Webpack build process (in `webpack.config.js`)
echo 📦 Install cargo-about
wget "https://github.com/EmbarkStudios/cargo-about/releases/download/0.5.5/cargo-about-0.5.5-x86_64-unknown-linux-musl.tar.gz"
tar -xzf cargo-about-0.5.5-x86_64-unknown-linux-musl.tar.gz
mv cargo-about-0.5.5-x86_64-unknown-linux-musl/cargo-about /opt/buildhome/.cargo/bin

# Install the wasm-pack Rust dependency that's used during the build process
echo 📦 Install wasm-pack
wget "https://github.com/rustwasm/wasm-pack/releases/download/v0.11.0/wasm-pack-v0.11.0-x86_64-unknown-linux-musl.tar.gz"
tar -xzf wasm-pack-v0.11.0-x86_64-unknown-linux-musl.tar.gz
mv wasm-pack-v0.11.0-x86_64-unknown-linux-musl/wasm-pack /opt/buildhome/.cargo/bin

wasm-pack --version
wasm-opt --version


# Install the project's Node dependencies through npm
echo 🚧 Install Node dependencies
echo node version:
node --version
echo npm version:
npm --version
cd frontend
npm ci --no-optional


# Build for production
echo 👷 Build Graphite web client
export NODE_ENV=production
npm run build && mv public dist # `&&` is used here to preserve the exit code
