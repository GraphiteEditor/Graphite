<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	import { type Editor as GraphiteEditor, initWasm, createEditor } from "@graphite/editor";

	import Editor from "@graphite/components/Editor.svelte";

	let editor: GraphiteEditor | undefined = undefined;

	let autoSaveAllDocumentsId: ReturnType<typeof setInterval> | undefined = undefined;
	let autoPanningId: ReturnType<typeof setInterval> | undefined = undefined;
	onMount(async () => {
		await initWasm();

		editor = createEditor();

		// Auto save every 15 seconds
		autoSaveAllDocumentsId = setInterval(() => {
			editor?.handle.autoSaveAllDocuments();
		}, 15000);

		// Check for autoPanning every 15ms
		autoPanningId = setInterval(() => {
			editor?.handle.autoPanning();
		}, 15);
	});

	onDestroy(() => {
		// Destroy the WASM editor handle
		editor?.handle.free();
		clearInterval(autoSaveAllDocumentsId);
		clearInterval(autoPanningId);
	});
</script>

{#if editor !== undefined}
	<Editor {editor} />
{/if}
