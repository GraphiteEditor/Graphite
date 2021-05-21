<template>
	<div class="status-bar">
		<UserInputLabel :inputMouse="'LMB'">Select Object</UserInputLabel>
		<span class="plus">+</span>
		<UserInputLabel :inputKeys="['⇧']">Grow/Shrink Selection</UserInputLabel>
		<div class="divider"></div>
		<UserInputLabel :inputMouse="'LMBDrag'">Select Area</UserInputLabel>
		<span class="plus">+</span>
		<UserInputLabel :inputKeys="['⇧']">Grow/Shrink Selection</UserInputLabel>
		<div class="divider"></div>
		<UserInputLabel :inputKeys="['G']">Grab Selected</UserInputLabel>
		<UserInputLabel :inputKeys="['R']">Rotate Selected</UserInputLabel>
		<UserInputLabel :inputKeys="['S']">Scale Selected</UserInputLabel>
		<div class="divider"></div>
		<UserInputLabel :inputKeys="['↑', '→', '↓', '←']">Nudge Selected</UserInputLabel>
		<span class="plus">+</span>
		<UserInputLabel :inputKeys="['⇧']">Big Increment Nudge</UserInputLabel>
		<div class="divider"></div>
		<UserInputLabel :inputKeys="['Alt']" :inputMouse="'LMBDrag'">Move Duplicate</UserInputLabel>
		<UserInputLabel :inputKeys="['Ctrl', 'D']">Duplicate</UserInputLabel>
	</div>
</template>

<style lang="scss">
.status-bar {
	display: flex;
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

.user-input-label + .user-input-label {
	margin-left: 0;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import UserInputLabel from "../../widgets/labels/UserInputLabel.vue";

export default defineComponent({
	components: {
		UserInputLabel,
	},
});
</script>
