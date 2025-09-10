<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";

	export let maximized;

	const editor = getContext<Editor>("editor");
</script>

<LayoutRow class="window-button windows" tooltip="Minimize" on:click={() => editor.handle.appWindowMinimize()}>
	<IconLabel icon="WindowButtonWinMinimize" />
</LayoutRow>
<LayoutRow class="window-button windows" tooltip={maximized ? "Restore Down" : "Maximize"} on:click={() => editor.handle.appWindowMaximize()}>
	<IconLabel icon={maximized ? "WindowButtonWinRestoreDown" : "WindowButtonWinMaximize"} />
</LayoutRow>
<LayoutRow class="window-button windows" tooltip="Close" on:click={() => editor.handle.appWindowClose()}>
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
			background: var(--color-6-lowergray);

			svg {
				fill: var(--color-f-white);
			}
		}

		&:last-of-type:hover {
			background: #e81123;
		}
	}
</style>
