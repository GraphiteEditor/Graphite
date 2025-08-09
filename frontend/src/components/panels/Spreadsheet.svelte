<script lang="ts">
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	import { getContext } from "svelte";

	import type { PortfolioState } from "/src/state-providers/portfolio";

	const portfolio = getContext<PortfolioState>("portfolio");
</script>

<LayoutCol class="spreadsheet">
	<LayoutCol class="body" scrollableY={true}>
		<WidgetLayout layout={$portfolio.spreadsheetWidgets} />
	</LayoutCol>
</LayoutCol>

<style lang="scss" global>
	.spreadsheet {
		flex-grow: 1;
		padding: 4px;

		table {
			margin: -4px;
			width: calc(100% + 2 * 4px);

			.text-label {
				white-space: wrap;
			}

			&:not(:first-child) {
				margin-top: 0;
			}
		}
	}
</style>
