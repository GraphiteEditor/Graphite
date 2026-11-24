<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import Editor from "/src/components/Editor.svelte";
	import { createSubscriptionsRouter } from "/src/subscriptions-router";
	import type { MessageName, SubscriptionsRouter } from "/src/subscriptions-router";
	import { joinSessionFromUrl, loadDemoArtwork } from "/src/utility-functions/network";
	import { operatingSystem } from "/src/utility-functions/platform";
	import { registerServiceWorker } from "/src/utility-functions/service-worker";
	import { initWasm } from "/src/utility-functions/wasm-loader";
	import { EditorWrapper, receiveNativeMessage } from "/wrapper/pkg/graphite_wasm_wrapper";
	import type { FrontendMessage } from "/wrapper/pkg/graphite_wasm_wrapper";

	let subscriptions: SubscriptionsRouter | undefined = undefined;
	let editor: EditorWrapper | undefined = undefined;
	// Why the editor couldn't start, such as the browser lacking the GPU support it renders with
	let startupError: string | undefined = undefined;

	onMount(async () => {
		try {
			// Initialize the editor wrapper
			const wrapper = await initWasm();

			// Register the service worker, except in dev mode and native builds
			if (!import.meta.env.DEV && import.meta.env.MODE !== "native" && "serviceWorker" in navigator) registerServiceWorker();

			for (const [name, f] of Object.entries(wrapper)) {
				if (name.startsWith("__node_registry") || name.startsWith("__migration_registry")) f();
			}
			// The Wasm module outlives a remount, along with the canvases it already registered
			window.imageCanvases ||= {};
			window.receiveNativeMessage = receiveNativeMessage;

			// Create the editor and subscriptions router
			const randomSeed = BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
			subscriptions = createSubscriptionsRouter();
			editor = await EditorWrapper.create(operatingSystem(), randomSeed, (messageType: MessageName, messageData: FrontendMessage) => {
				subscriptions?.handleFrontendMessage(messageType, messageData);
			});
		} catch (error) {
			// eslint-disable-next-line no-console
			console.error(error);
			startupError = error instanceof Error ? error.message : String(error);
			return;
		}

		await loadDemoArtwork(editor);
		joinSessionFromUrl(editor);
	});

	onDestroy(() => {
		editor?.free();
	});
</script>

{#if startupError !== undefined}
	<div class="startup-error">
		<h1>Graphite can't start</h1>
		<p>{startupError}</p>
	</div>
{:else if subscriptions !== undefined && editor !== undefined}
	<Editor {subscriptions} {editor} />
{/if}

<style lang="scss">
	// Disable the loading indicator
	body:has(.main-window, .startup-error)::before,
	body:has(.main-window, .startup-error)::after {
		content: none !important;
	}

	.startup-error {
		position: absolute;
		inset: 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 16px;
		padding: 32px;
		background: var(--color-2-mildblack);
		text-align: center;
		text-wrap: balance;
		user-select: text;

		h1 {
			margin: 0;
			font-size: 24px;
			font-weight: 700;
		}

		p {
			margin: 0;
			max-width: 480px;
			line-height: 1.5;
			color: var(--color-b-lightgray);
		}
	}
</style>
