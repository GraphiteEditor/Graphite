<script lang="ts">
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";

	export let maximized = false;
</script>

<LayoutRow class="window-button windows minimize" tooltip="Minimize">
	<IconLabel icon={"WindowButtonWinMinimize"} />
</LayoutRow>
{#if !maximized}
	<LayoutRow class="window-button windows maximize" tooltip="Maximize">
		<IconLabel icon={"WindowButtonWinMaximize"} />
	</LayoutRow>
{:else}
	<LayoutRow class="window-button windows restore-down" tooltip="Restore Down">
		<IconLabel icon={"WindowButtonWinRestoreDown"} />
	</LayoutRow>
{/if}
<LayoutRow class="window-button windows close" tooltip="Close">
	<IconLabel icon={"WindowButtonWinClose"} />
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

		&.close:hover {
			background: #e81123;
		}
	}
</style>
