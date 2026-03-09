<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	import { initWasm, createEditor } from "@graphite/editor";
	import type { Editor as GraphiteEditor } from "@graphite/editor";

	import Editor from "@graphite/components/Editor.svelte";

	let editor: GraphiteEditor | undefined = undefined;

	onMount(async () => {
		await initWasm();

		editor = createEditor();
	});

	onDestroy(() => {
		// Destroy the Wasm editor handle
		editor?.handle.free();
	});
</script>

{#if editor !== undefined}
	<Editor {editor} />
{/if}
