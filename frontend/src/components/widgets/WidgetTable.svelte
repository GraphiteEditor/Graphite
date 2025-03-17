<script lang="ts">
	import { type WidgetTable as WidgetTableFromJsMessages } from "@graphite/messages";

	import WidgetSpan from "@graphite/components/widgets/WidgetSpan.svelte";

	export let widgetData: WidgetTableFromJsMessages;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	export let layoutTarget: any; // TODO: Give type
</script>

<table border="1">
	{#each widgetData.tableWidgets as row}
		<tr>
			{#each row as cell}
				<td>
					<WidgetSpan widgetData={{ rowWidgets: [cell] }} {layoutTarget} />
				</td>
			{/each}
		</tr>
	{/each}
</table>

<style lang="scss">
	table {
		max-width: 100%;
		display: block;
	}
</style>
