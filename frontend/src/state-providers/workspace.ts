/* eslint-disable max-classes-per-file */

import {writable} from "svelte/store";

import { type Editor } from "~/src/wasm-communication/editor";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createWorkspaceState(editor: Editor) {
	const { subscribe, update } = writable({});

	// Set up message subscriptions on creation


	return {
		subscribe,
	};
}
export type WorkspaceState = ReturnType<typeof createWorkspaceState>;
