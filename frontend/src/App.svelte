<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	import { type Editor as GraphiteEditor, initWasm, createEditor } from "@graphite/editor";

	import Editor from "@graphite/components/Editor.svelte";

	import { send_message_to_cef } from "/wasm/pkg/graphite_wasm";

	let editor: GraphiteEditor | undefined = undefined;

	onMount(async () => {
		await initWasm();

		editor = createEditor();
	});

	onDestroy(() => {
		// Destroy the WASM editor handle
		editor?.handle.free();
	});

	console.log("Test from app.svelte javascript");
	sendMessageToCef("Test from app direct");
</script>

{#if editor !== undefined}
	<Editor {editor} />
{/if}
