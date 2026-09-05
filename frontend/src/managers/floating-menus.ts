import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

const openFloatingMenus: Set<string> = import.meta.hot?.data?.openFloatingMenus || new Set();
if (import.meta.hot) import.meta.hot.data.openFloatingMenus = openFloatingMenus;

let editorWrapper: EditorWrapper | undefined = undefined;
let sendDirectInput: ((enabled: boolean) => void) | undefined = undefined;

export function createFloatingMenusManager(editor: EditorWrapper) {
	editorWrapper = editor;
	sendDirectInput = (enabled) => editor.appWindowDirectInput(enabled);
	sendDirectInput(openFloatingMenus.size === 0);
}

export function destroyFloatingMenusManager() {
	sendDirectInput?.(false);
	sendDirectInput = undefined;
}

export function reportFloatingMenuOpen(menuId: string) {
	openFloatingMenus.add(menuId);
	sendDirectInput?.(openFloatingMenus.size <= 0);
}

export function reportFloatingMenuClose(menuId: string) {
	openFloatingMenus.delete(menuId);
	sendDirectInput?.(openFloatingMenus.size <= 0);
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (editorWrapper) newModule?.createFloatingMenusManager(editorWrapper);
});
