<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	import init, { EditorHandle, receiveNativeMessage } from "@graphite/../wasm/pkg/graphite_wasm";
	import type { FrontendMessage } from "@graphite/../wasm/pkg/graphite_wasm";
	import { loadDemoArtwork } from "@graphite/utility-functions/network";
	import { operatingSystem } from "@graphite/utility-functions/platform";
	import { createSubscriptionsRouter } from "/src/subscriptions-router";
	import type { MessageName, SubscriptionsRouter } from "/src/subscriptions-router";

	import Editor from "@graphite/components/Editor.svelte";

	let subscriptions: SubscriptionsRouter | undefined = undefined;
	let editor: EditorHandle | undefined = undefined;

	onMount(async () => {
		// Initialize the Wasm module
		const wasm = await init();
		for (const [name, f] of Object.entries(wasm)) {
			if (name.startsWith("__node_registry")) f();
		}
		window.imageCanvases = {};
		window.receiveNativeMessage = receiveNativeMessage;

		// Create the editor and subscriptions router
		const randomSeed = BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
		subscriptions = createSubscriptionsRouter();
		editor = EditorHandle.create(operatingSystem(), randomSeed, (messageType: MessageName, messageData: FrontendMessage) => {
			subscriptions?.handleFrontendMessage(messageType, messageData);
		});

		await loadDemoArtwork(editor);
	});

	onDestroy(() => {
		editor?.free();
	});
</script>

{#if subscriptions !== undefined && editor !== undefined}
	<Editor {subscriptions} {editor} />
{/if}
