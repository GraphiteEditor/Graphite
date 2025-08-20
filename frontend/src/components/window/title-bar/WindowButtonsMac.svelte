<script lang="ts">
	import { getContext } from "svelte";
	import type { Editor } from "@graphite/editor";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	const editor = getContext<Editor>("editor");
</script>

<LayoutRow class="window-buttons-mac">
	<button
		type="button"
		class="close"
		aria-label="Close window"
		on:click={() => editor.handle.appWindowClose()}
	/>
	<button
		type="button"
		class="minimize"
		aria-label="Minimize window"
		on:click={() => editor.handle.appWindowMinimize()}
	/>
	<button
		type="button"
		class="zoom"
		aria-label="Maximize window"
		on:click={() => editor.handle.appWindowMaximize()}
	/>
</LayoutRow>

<style lang="scss" global>
	.window-buttons-mac {
		flex: 0 0 auto;
		align-items: center;
		margin: 0 8px;

		button {
			all: unset;                                                              
			cursor: pointer;
			flex: 0 0 auto;
			align-items: center;
			width: 11px;
			height: 11px;
			border-radius: 50%;

			& + button {
				margin-left: 8px;
			}

			&.close {
				background: #ff5a52;
			}

			&.minimize {
				background: #e6c029;
			}

			&.zoom {
				background: #54c22b;
			}
		}
	}
</style>
