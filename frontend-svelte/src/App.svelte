<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	import { initWasm, createEditor } from "@/wasm-communication/editor";

	import Editor from "@/components/Editor.svelte";

	console.log("App.svelte: entry start");

	let editor: ReturnType<typeof createEditor> | undefined = undefined;

	console.log("App.svelte: entry end");

	onMount(async () => {
		console.log("App.svelte: onMount start (calling initWasm())");

		await initWasm();

		console.log("App.svelte: onMount (calling createEditor())");

		editor = createEditor();

		console.log("App.svelte: onMount end (done calling createEditor())");
	});

	onDestroy(() => {
		console.log("App.svelte: onDestroy start");

		// Destroy the WASM editor instance
		editor?.instance.free();

		console.log("App.svelte: onDestroy end");
	});
</script>

{#if editor !== undefined}
	<Editor {editor} />
{/if}
