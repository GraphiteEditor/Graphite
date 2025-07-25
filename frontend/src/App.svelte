<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	import { type Editor as GraphiteEditor, initWasm, createEditor } from "@graphite/editor";

	import Editor from "@graphite/components/Editor.svelte";

	let editor: GraphiteEditor | undefined = undefined;

	onMount(async () => {
		await initWasm();

		editor = createEditor();
	});

	onDestroy(() => {
		// Destroy the WASM editor handle
		editor?.handle.free();
	});
</script>

{#if editor !== undefined}
	<Editor {editor} />
{/if}
