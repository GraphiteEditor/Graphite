import fs from "fs/promises"
import path from "path"
import child_process from "child_process"
import assert from "assert"

// change $PWD to script's directory
process.chdir(path.dirname(new URL(import.meta.url).pathname))

const p0 = child_process.spawn("wasm-pack", ["build"], { stdio: "inherit" })
p0.on("exit", (code) => {
  assert.equal(code, 0)
  process.chdir("pkg")
  const p1 = child_process.spawn("npm", ["add", "../../frontend/glue"], {
    stdio: "inherit",
  })
  p1.on("exit", (code) => {
    assert.equal(code, 0)
  })
})
