import { DialogState } from "@/state/dialog";
import { DocumentsState } from "@/state/documents";
import { FullscreenState } from "@/state/fullscreen";
import { EditorState } from "@/state/wasm-loader";
import { InputManager } from "@/utilities/input";

// Allow JS import statements to work with .vue files
declare module "*.vue" {
	const component: DefineComponent;
	export default component;
}

// Allow JS import statements to work with .svg files
declare module "*.svg" {
	const component: DefineComponent;
	export default component;
}

// Vue injects don't play well with TypeScript, and all injects will show up as `any`. As a workaround, we can define these types.
declare module "@vue/runtime-core" {
	interface ComponentCustomProperties {
		dialog: DialogState;
		documents: DocumentsState;
		fullscreen: FullscreenState;
		editor: EditorState;
		inputManger?: InputManager;
	}
}
