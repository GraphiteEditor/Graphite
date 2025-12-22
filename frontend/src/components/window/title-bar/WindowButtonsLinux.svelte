<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { AppWindowState } from "@graphite/state-providers/app-window";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";

	const appWindow = getContext<AppWindowState>("appWindow");
	const editor = getContext<Editor>("editor");
</script>

<LayoutRow class="window-button linux" tooltipLabel="Minimize" on:click={() => editor.handle.appWindowMinimize()}>
	<IconLabel icon="WindowButtonWinMinimize" />
</LayoutRow>
<LayoutRow class="window-button linux" tooltipLabel={$appWindow.maximized ? "Unmaximize" : "Maximize"} on:click={() => editor.handle.appWindowMaximize()}>
	<IconLabel icon={$appWindow.maximized ? "WindowButtonWinRestoreDown" : "WindowButtonWinMaximize"} />
</LayoutRow>
<LayoutRow class="window-button linux" tooltipLabel="Close" on:click={() => editor.handle.appWindowClose()}>
	<IconLabel icon="WindowButtonWinClose" />
</LayoutRow>

<style lang="scss" global>
	.window-button.linux {
		flex: 0 0 auto;
		align-items: center;
		padding: 0 12px;

		svg {
			fill: var(--color-e-nearwhite);
		}

		&:hover {
			background: var(--color-6-lowergray);
			border-radius: 2px;

			svg {
				fill: var(--color-f-white);
			}
		}
	}
</style>
