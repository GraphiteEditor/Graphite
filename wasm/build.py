import os, json

def patch_json():
    os.chdir("pkg")
    assert os.system("npm add ../../frontend-svelte") == 0
assert os.system("wasm-pack build") == 0
patch_json()
