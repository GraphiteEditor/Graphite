<script lang="ts">
	import { getContext } from "svelte";

	import LayoutRow from "@/components/layout/LayoutRow.svelte";
	import IconLabel from "@/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@/components/widgets/labels/TextLabel.svelte";
	import type { FullscreenState } from "@/state-providers/fullscreen";

	const fullscreen = getContext<FullscreenState>("fullscreen");

	$: windowFullscreen = $fullscreen.windowFullscreen;
	$: requestFullscreenHotkeys = fullscreen.keyboardLockApiSupported && !$fullscreen.keyboardLocked;

	async function handleClick() {
		if (windowFullscreen) fullscreen.exitFullscreen();
		else fullscreen.enterFullscreen();
	}
</script>

<LayoutRow class="window-buttons-web" on:click={() => handleClick()} tooltip={(windowFullscreen ? "Exit" : "Enter") + " Fullscreen (F11)"}>
	{#if requestFullscreenHotkeys}
		<TextLabel italic={true}>Go fullscreen to access all hotkeys</TextLabel>
	{/if}
	<IconLabel icon={windowFullscreen ? "FullscreenExit" : "FullscreenEnter"} />
</LayoutRow>

<style lang="scss" global>
	.window-buttons-web {
		flex: 0 0 auto;
		align-items: center;
		padding: 0 8px;

		svg {
			fill: var(--color-e-nearwhite);
		}

		.text-label {
			margin-right: 8px;
		}

		&:hover {
			background: var(--color-6-lowergray);
			color: var(--color-f-white);

			svg {
				fill: var(--color-f-white);
			}
		}
	}
</style>
