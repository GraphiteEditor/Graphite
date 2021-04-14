<template>
	<div class="hint-area footer-area">
		<InputHint :inputMouse="'LMB'">Select Object</InputHint>
		<span class="plus">+</span>
		<InputHint :inputKeys="['⇧']">Grow/Shrink Selection</InputHint>
		<div class="divider"></div>
		<InputHint :inputMouse="'LMBDrag'">Select Area</InputHint>
		<span class="plus">+</span>
		<InputHint :inputKeys="['⇧']">Grow/Shrink Selection</InputHint>
		<div class="divider"></div>
		<InputHint :inputKeys="['G']">Grab Selected</InputHint>
		<InputHint :inputKeys="['R']">Rotate Selected</InputHint>
		<InputHint :inputKeys="['S']">Scale Selected</InputHint>
		<div class="divider"></div>
		<InputHint :inputKeys="['↑', '→', '↓', '←']">Nudge Selected</InputHint>
		<span class="plus">+</span>
		<InputHint :inputKeys="['⇧']">Big Increment Nudge</InputHint>
		<div class="divider"></div>
		<InputHint :inputKeys="['Alt']" :inputMouse="'LMBDrag'">Move Duplicate</InputHint>
		<InputHint :inputKeys="['Ctrl', 'D']">Duplicate</InputHint>
	</div>
	<div class="status-area footer-area">
		<div>
			<span>Graphite 0.0.1</span>
		</div>
	</div>
</template>

<style lang="scss">
.footer-area {
	display: flex;

	&.hint-area {
		flex: 1 1 100%;
	}

	&.status-area div {
		margin: 0 8px;
		display: flex;
		align-items: center;
		white-space: nowrap;
	}
}

.plus {
	display: flex;
	align-items: center;
	font-weight: bold;
}

.divider {
	width: 1px;
	height: 16px;
	margin: 4px 0;
	background: #888;
}

.input-hint + .input-hint {
	margin-left: 0;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import InputHint from "./InputHint.vue";

export default defineComponent({
	components: {
		InputHint,
	},
});
</script>
