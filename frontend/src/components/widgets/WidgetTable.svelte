<script lang="ts">
	import WidgetSpan from "/src/components/widgets/WidgetSpan.svelte";
	import type { LayoutTarget, WidgetTable } from "/wrapper/pkg/graphite_wasm_wrapper";

	export let widgetData: WidgetTable;
	export let layoutTarget: LayoutTarget;

	$: columns = widgetData.tableWidgets.length > 0 ? widgetData.tableWidgets[0].length : 0;
</script>

<table class:unstyled={widgetData.unstyled}>
	<tbody>
		{#each widgetData.tableWidgets as row}
			<tr>
				{#each row as cell}
					<td colspan={row.length < columns ? columns - row.length + 1 : undefined}>
						<WidgetSpan direction="row" widgets={[cell]} {layoutTarget} narrow={true} />
					</td>
				{/each}
			</tr>
		{/each}
	</tbody>
</table>

<style lang="scss" global>
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
