<template>
	<div class="status-bar">
		<UserInputLabel :inputMouse="'LMBDrag'">Drag Selected</UserInputLabel>
		<Separator :type="SeparatorType.Section" />
		<UserInputLabel :inputKeys="[['G']]">Grab Selected</UserInputLabel>
		<UserInputLabel :inputKeys="[['R']]">Rotate Selected</UserInputLabel>
		<UserInputLabel :inputKeys="[['S']]">Scale Selected</UserInputLabel>
		<Separator :type="SeparatorType.Section" />
		<UserInputLabel :inputMouse="'LMB'">Select Object</UserInputLabel>
		<span class="plus">+</span>
		<UserInputLabel :inputKeys="[['Ctrl']]">Innermost</UserInputLabel>
		<span class="plus">+</span>
		<UserInputLabel :inputKeys="[['⇧']]">Grow/Shrink Selection</UserInputLabel>
		<Separator :type="SeparatorType.Section" />
		<UserInputLabel :inputMouse="'LMBDrag'">Select Area</UserInputLabel>
		<span class="plus">+</span>
		<UserInputLabel :inputKeys="[['⇧']]">Grow/Shrink Selection</UserInputLabel>
		<Separator :type="SeparatorType.Section" />
		<UserInputLabel :inputKeys="[['↑'], ['→'], ['↓'], ['←']]">Nudge Selected</UserInputLabel>
		<span class="plus">+</span>
		<UserInputLabel :inputKeys="[['⇧']]">Big Increment Nudge</UserInputLabel>
		<Separator :type="SeparatorType.Section" />
		<UserInputLabel :inputKeys="[['Alt']]" :inputMouse="'LMBDrag'">Move Duplicate</UserInputLabel>
		<UserInputLabel :inputKeys="[['Ctrl', 'D']]">Duplicate</UserInputLabel>
	</div>
</template>

<style lang="scss">
.status-bar {
	display: flex;
	flex-wrap: wrap;
	margin: 0 -4px;
	// TODO: Use CSS grid to solve issue that makes overflowed items have inconsistent left padding on second row when overflowed

	> * {
		height: 24px;
	}

	.separator.section {
		height: 24px;
		margin: 0;
	}

	.plus {
		display: flex;
		align-items: center;
		font-weight: bold;
	}

	.user-input-label + .user-input-label {
		margin-left: 0;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import UserInputLabel from "@/components/widgets/labels/UserInputLabel.vue";
import Separator, { SeparatorType } from "@/components/widgets/separators/Separator.vue";

export default defineComponent({
	components: {
		UserInputLabel,
		Separator,
	},
	data() {
		return {
			SeparatorType,
		};
	},
});
</script>
