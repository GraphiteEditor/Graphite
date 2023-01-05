#!/usr/bin/env node

import fs from "fs/promises"
import path from "path"
import child_process from "child_process"
import assert from "assert"

let devMode = false
switch (process.env.NODE_ENV) {
  case undefined:
    console.error("No NODE_ENV, I bail!")
    console.error("Valid values: 'development' or 'production'")
    process.exit(1)
    break
  case "development":
    devMode = true
    break
  case "production":
    break
  default:
    console.error("Invalid NODE_ENV, I also bail!")
    console.error("Valid values: 'development' or 'production'")
    process.exit(1)
    break
}

// change $PWD to script's directory
// process.chdir(path.dirname(new URL(import.meta.url).pathname))

const extra_args = devMode ? ["--dev"] : []
const p0 = child_process.spawn("wasm-pack", ["build", ...extra_args], {
  stdio: "inherit",
})
p0.on("exit", (code) => {
  assert.equal(code, 0)
  process.chdir("pkg")
  const command = /^win/.test(process.platform) ? 'npm.cmd' : 'npm';
  const p1 = child_process.spawn(command, ["add", "../../frontend/glue"], {
    stdio: "inherit",
  })
  p1.on("exit", (code) => {
    assert.equal(code, 0)
  })
})
