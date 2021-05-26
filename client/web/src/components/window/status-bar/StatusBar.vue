<template>
	<div class="status-bar">
		<UserInputLabel :inputMouse="'LMBDrag'">Drag Selected</UserInputLabel>
		<Separator :type="SeparatorType.Section" />
		<UserInputLabel :inputKeys="['G']">Grab Selected</UserInputLabel>
		<UserInputLabel :inputKeys="['R']">Rotate Selected</UserInputLabel>
		<UserInputLabel :inputKeys="['S']">Scale Selected</UserInputLabel>
		<Separator :type="SeparatorType.Section" />
		<UserInputLabel :inputMouse="'LMB'">Select Object</UserInputLabel>
		<span class="plus">+</span>
		<UserInputLabel :inputKeys="['Ctrl']">Innermost</UserInputLabel>
		<span class="plus">+</span>
		<UserInputLabel :inputKeys="['⇧']">Grow/Shrink Selection</UserInputLabel>
		<Separator :type="SeparatorType.Section" />
		<UserInputLabel :inputMouse="'LMBDrag'">Select Area</UserInputLabel>
		<span class="plus">+</span>
		<UserInputLabel :inputKeys="['⇧']">Grow/Shrink Selection</UserInputLabel>
		<Separator :type="SeparatorType.Section" />
		<UserInputLabel :inputKeys="['↑', '→', '↓', '←']">Nudge Selected</UserInputLabel>
		<span class="plus">+</span>
		<UserInputLabel :inputKeys="['⇧']">Big Increment Nudge</UserInputLabel>
		<Separator :type="SeparatorType.Section" />
		<UserInputLabel :inputKeys="['Alt']" :inputMouse="'LMBDrag'">Move Duplicate</UserInputLabel>
		<UserInputLabel :inputKeys="['Ctrl', 'D']">Duplicate</UserInputLabel>
	</div>
</template>

<style lang="scss">
.status-bar {
	display: flex;

	.plus {
		display: flex;
		align-items: center;
		font-weight: bold;
	}

	.user-input-label + .user-input-label {
		margin-left: 0;
	}

	.separator.section {
		margin: 0;
	}

	> :first-child {
		margin-left: 4px;
	}

	> :last-child {
		margin-right: 4px;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import UserInputLabel from "../../widgets/labels/UserInputLabel.vue";
import Separator, { SeparatorType } from "../../widgets/Separator.vue";

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
