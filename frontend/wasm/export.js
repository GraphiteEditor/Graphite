import fs from "node:fs";
import path from "node:path";
import graphite, { get_specta_types } from "./pkg/graphite_wasm.js";

graphite(fs.readFileSync(path.join(import.meta.dirname, './pkg/graphite_wasm_bg.wasm'))).then(() =>
	fs.writeFileSync("bindings_from_node.ts", get_specta_types())
);
