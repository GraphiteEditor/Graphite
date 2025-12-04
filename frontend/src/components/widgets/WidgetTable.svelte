<script lang="ts">
	import { type WidgetTable as WidgetTableFromJsMessages } from "@graphite/messages";

	import WidgetSpan from "@graphite/components/widgets/WidgetSpan.svelte";

	export let widgetData: WidgetTableFromJsMessages;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	export let layoutTarget: any;
	export let unstyled = false;

	$: columns = widgetData.tableWidgets.length > 0 ? widgetData.tableWidgets[0].length : 0;
</script>

<table class:unstyled>
	<tbody>
		{#each widgetData.tableWidgets as row}
			<tr>
				{#each row as cell}
					<td colspan={row.length < columns ? columns - row.length + 1 : undefined}>
						<WidgetSpan widgetData={{ rowWidgets: [cell] }} {layoutTarget} narrow={true} />
					</td>
				{/each}
			</tr>
		{/each}
	</tbody>
</table>

<style lang="scss">
	table:not(.unstyled) {
		background: var(--color-3-darkgray);
		border: none;
		border-spacing: 4px;
		border-radius: 2px;

		td {
			background: var(--color-2-mildblack);
			vertical-align: top;
			border: none;
			border-radius: 2px;
			padding: 4px 8px;
		}

		tr:first-child td {
			background-image: var(--inheritance-dots-background-4-dimgray);
		}
	}
</style>
