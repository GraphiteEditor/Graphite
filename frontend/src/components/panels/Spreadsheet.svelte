<script lang="ts">
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	import { getContext } from "svelte";

	import type { PortfolioState } from "/src/state-providers/portfolio";

	const portfolio = getContext<PortfolioState>("portfolio");
</script>

<section id="spreadsheet-main">
	<h1>Spreadsheet for node id {$portfolio.spreadsheetNode}:</h1>
	<WidgetLayout layout={$portfolio.spreadsheetWidgets} />
</section>

<style lang="scss" global>
	#spreadsheet-main {
		overflow-y: scroll;
		flex-grow: 1;

		span {
			white-space: wrap;
		}
	}
</style>
