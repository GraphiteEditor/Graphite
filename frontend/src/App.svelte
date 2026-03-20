<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	import type { EditorHandle } from "@graphite/../wasm/pkg/graphite_wasm";
	import { initWasm, createEditor } from "@graphite/editor";
	import type { SubscriptionsRouter } from "/src/subscriptions-router";

	import Editor from "@graphite/components/Editor.svelte";

	let subscriptions: SubscriptionsRouter | undefined = undefined;
	let editor: EditorHandle | undefined = undefined;
	let destroy: (() => void) | undefined = undefined;

	onMount(async () => {
		await initWasm();

		const created = createEditor();
		subscriptions = created.subscriptions;
		editor = created.editor;
		destroy = created.destroy;
	});

	onDestroy(() => {
		destroy?.();
	});
</script>

{#if subscriptions !== undefined && editor !== undefined}
	<Editor {subscriptions} {editor} />
{/if}
