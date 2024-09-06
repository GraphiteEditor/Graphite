<script lang="ts">
	import { getContext } from "svelte";

	import type { PortfolioState } from "@graphite/state-providers/portfolio";
	import type { Editor } from "@graphite/wasm-communication/editor";
	import type { FrontendDocumentDetails } from "@graphite/wasm-communication/messages";
	import { type DivisionOrPanel, type Tab } from "@graphite/wasm-communication/messages";

	import Division from "@graphite/components/window/workspace/Division.svelte";
	import Panel from "@graphite/components/window/workspace/Panel.svelte";

	export let value: DivisionOrPanel;

	// $: if ("Panel" in value) {
	// 	console.info(value.Panel.tabs[value.Panel.activeIndex]);
	// }

	function tabLabel(tab: Tab, documents: FrontendDocumentDetails[]) {
		if (tab.tabData === undefined) return { name: tab.tabType };
		const document = documents.find((document) => document.id === tab.tabData?.documentId);
		if (document === undefined) return { name: tab.tabType };
		const name = `${document.name}${document.isSaved ? "" : "*"}`;

		if (!editor.handle.inDevelopmentMode()) return { name };

		const tooltip = `Document ID: ${document.id}`;
		return { name, tooltip };
	}

	const editor = getContext<Editor>("editor");
	const portfolio = getContext<PortfolioState>("portfolio");
</script>

{#if "Division" in value}
	<Division divisionData={value.Division} />
{:else}
	<Panel
		panelType={value.Panel.tabs[value.Panel.activeIndex]?.tabType}
		panelIdentifier={value.Panel.identifier}
		tabLabels={value.Panel.tabs.map((tab) => tabLabel(tab, $portfolio.documents))}
		tabActiveIndex={Number(value.Panel.activeIndex)}
	/>
{/if}
