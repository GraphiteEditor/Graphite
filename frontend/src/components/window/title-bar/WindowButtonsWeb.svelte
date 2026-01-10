<script lang="ts">
	import { getContext } from "svelte";

	import type { FullscreenState } from "@graphite/state-providers/fullscreen";

	import type { TooltipState } from "@graphite/state-providers/tooltip";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";

	const fullscreen = getContext<FullscreenState>("fullscreen");
	const tooltip = getContext<TooltipState>("tooltip");

	async function handleClick() {
		if ($fullscreen.windowFullscreen) fullscreen.exitFullscreen();
		else fullscreen.enterFullscreen();
	}
</script>

<LayoutRow
	class="window-buttons-web"
	on:click={handleClick}
	tooltipLabel={$fullscreen.windowFullscreen ? "Exit Fullscreen" : "Enter Fullscreen"}
	tooltipDescription={$fullscreen.keyboardLockApiSupported ? "While fullscreen, keyboard shortcuts normally reserved by the browser become available." : ""}
	tooltipShortcut={$tooltip.f11Shortcut}
>
	<IconLabel icon={$fullscreen.windowFullscreen ? "FullscreenExit" : "FullscreenEnter"} />
</LayoutRow>

<style lang="scss" global>
	.window-buttons-web {
		flex: 0 0 auto;
		align-items: center;
		padding: 0 8px;

		svg {
			fill: var(--color-e-nearwhite);
		}

		&:hover {
			background: var(--color-6-lowergray);

			svg {
				fill: var(--color-f-white);
			}
		}
	}
</style>
