import os, json

def patch_json():
    manifest = json.load(open("pkg/package.json"))
    if "dependencies" in manifest:
        return
    manifest["dependencies"] = {
        "graphite-frontend": "file:../../frontend"
    }
    json.dump(manifest, open("pkg/package.json", "w"))

assert os.system("wasm-pack build") == 0
patch_json()