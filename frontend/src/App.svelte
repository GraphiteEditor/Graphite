<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	import { initWasm, createEditor } from "@graphite/wasm-communication/editor";

	import Editor from "@graphite/components/Editor.svelte";

	let editor: ReturnType<typeof createEditor> | undefined = undefined;

	onMount(async () => {
		await initWasm();

		editor = createEditor();
	});

	onDestroy(() => {
		// Destroy the WASM editor instance
		editor?.instance.free();
	});
</script>

{#if editor !== undefined}
	<Editor {editor} />
{/if}
