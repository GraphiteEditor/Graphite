<script lang="ts">
	import { getContext } from "svelte";

	import type { PortfolioState } from "/src/state-providers/portfolio";

	const portfolio = getContext<PortfolioState>("portfolio");
</script>

<section>
	<h1>Spreadsheet for node id {$portfolio.spreadsheetData?.inspectNode}:</h1>
	<span>{$portfolio.spreadsheetData?.data}</span>
</section>

<style lang="scss">
	span {
		white-space: pre;
	}
	section {
		overflow-y: scroll;
	}
</style>
