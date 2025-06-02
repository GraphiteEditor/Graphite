+++
title = "Knowing your tooling"

[extra]
order = 2 # Page number after chapter intro
+++

## First time builds

Slower.

## Troubleshooting

Delete the `target`, `pkg`, `node_modules`, and `dist` directories.

## Slow builds

If you're seeing the terminal spend several seconds installing wasm-opt every recompilation, reinstall the exact version of `wasm-bindgen-cli` that matches the `wasm-bindgen` dependency in [`Cargo.toml`](https://github.com/GraphiteEditor/Graphite/blob/master/Cargo.toml).

## Rust Analyzer speed tips
