<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	import { initWasm, createEditor } from "@graphite/editor";
	import type { Editor as GraphiteEditor } from "@graphite/editor";
	import type { SubscriptionRouter } from "@graphite/subscription-router";

	import Editor from "@graphite/components/Editor.svelte";

	let subscriptions: SubscriptionRouter | undefined = undefined;
	let editor: GraphiteEditor | undefined = undefined;

	onMount(async () => {
		await initWasm();

		const created = createEditor();
		subscriptions = created.subscriptions;
		editor = created.editor;
	});

	onDestroy(() => {
		editor?.destroy();
	});
</script>

{#if subscriptions !== undefined && editor !== undefined}
	<Editor {subscriptions} {editor} />
{/if}
