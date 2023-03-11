<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	import { initWasm, createEditor } from "./wasm-communication/editor";

	import Editor from "./components/Editor.svelte";

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
