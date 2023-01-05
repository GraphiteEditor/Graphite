import { browser } from "$app/environment";
import type { GraphiteEmitter } from "glue/emitter_type";
import type { JsEditorHandle } from "graphite-frontend-glue/editor";
import { writable, type Writable } from "svelte/store";

export let editor: Writable<JsEditorHandle | undefined> = writable(undefined);
export let pubsub: Writable<GraphiteEmitter | undefined> = writable(undefined);

export async function initEditor() {
	const { editor: _editor, editor_pubsub } = await import("graphite-frontend-glue/editor");
	editor.set(_editor);
	pubsub.set(editor_pubsub);
}

if (browser) {
	initEditor();
}
