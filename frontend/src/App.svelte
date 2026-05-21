<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import Editor from "/src/components/Editor.svelte";
	import { createSubscriptionsRouter } from "/src/subscriptions-router";
	import type { MessageName, SubscriptionsRouter } from "/src/subscriptions-router";
	import { loadDemoArtwork } from "/src/utility-functions/network";
	import { operatingSystem } from "/src/utility-functions/platform";
	import init, { EditorWrapper, receiveNativeMessage } from "/wrapper/pkg/graphite_wasm_wrapper";
	import type { FrontendMessage } from "/wrapper/pkg/graphite_wasm_wrapper";

	let subscriptions: SubscriptionsRouter | undefined = undefined;
	let editor: EditorWrapper | undefined = undefined;

	onMount(async () => {
		// Initialize the editor wrapper
		const wrapper = await init();
		for (const [name, f] of Object.entries(wrapper)) {
			if (name.startsWith("__node_registry")) f();
		}
		window.imageCanvases = {};
		window.receiveNativeMessage = receiveNativeMessage;

		// Create the editor and subscriptions router
		const randomSeed = BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
		subscriptions = createSubscriptionsRouter();
		editor = EditorWrapper.create(operatingSystem(), randomSeed, (messageType: MessageName, messageData: FrontendMessage) => {
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
