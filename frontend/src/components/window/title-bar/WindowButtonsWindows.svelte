<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { AppWindowState } from "@graphite/state-providers/app-window";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";

	const appWindow = getContext<AppWindowState>("appWindow");
	const editor = getContext<Editor>("editor");
</script>

<LayoutRow class="window-button windows" tooltipLabel="Minimize" on:click={() => editor.handle.appWindowMinimize()}>
	<IconLabel icon="WindowButtonWinMinimize" />
</LayoutRow>
<LayoutRow class="window-button windows" tooltipLabel={$appWindow.maximized ? "Restore Down" : "Maximize"} on:click={() => editor.handle.appWindowMaximize()}>
	<IconLabel icon={$appWindow.maximized ? "WindowButtonWinRestoreDown" : "WindowButtonWinMaximize"} />
</LayoutRow>
<LayoutRow class="window-button windows" tooltipLabel="Close" on:click={() => editor.handle.appWindowClose()}>
	<IconLabel icon="WindowButtonWinClose" />
</LayoutRow>

<style lang="scss" global>
	.window-button.windows {
		flex: 0 0 auto;
		align-items: center;
		padding: 0 17px;

		svg {
			fill: var(--color-e-nearwhite);
		}

		&:hover {
			background: #2d2d2d;

			svg {
				fill: var(--color-f-white);
			}
		}

		&:last-of-type:hover {
			background: #c42b1c;
		}
	}
</style>
