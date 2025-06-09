<script lang="ts">
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	import { getContext } from "svelte";

	import type { PortfolioState } from "/src/state-providers/portfolio";

	const portfolio = getContext<PortfolioState>("portfolio");
</script>

<LayoutCol class="spreadsheet">
	<LayoutRow class="control-bar">
		<TextLabel>Spreadsheet Data for Node ID {$portfolio.spreadsheetNode}:</TextLabel>
	</LayoutRow>
	<LayoutCol class="body" scrollableY={true}>
		<WidgetLayout layout={$portfolio.spreadsheetWidgets} />
	</LayoutCol>
</LayoutCol>

<style lang="scss" global>
	.spreadsheet {
		flex-grow: 1;
		padding: 4px;

		.control-bar {
			height: 32px;
			--widget-height: 24px;
			flex: 0 0 auto;

			.text-label {
				margin: calc((24px - var(--widget-height)) / 2 + 4px) 0;
				min-height: var(--widget-height);
				line-height: var(--widget-height);
			}
		}

		table {
			margin: 0 -4px;
			width: calc(100% + 2 * 4px);
			margin-top: 8px;

			.text-label {
				white-space: wrap;
			}
		}
	}
</style>
