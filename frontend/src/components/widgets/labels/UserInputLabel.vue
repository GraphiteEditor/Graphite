<template>
	<div class="user-input-label">
		<template v-for="(keyGroup, keyGroupIndex) in inputKeys" :key="keyGroupIndex">
			<span class="group-gap" v-if="keyGroupIndex > 0"></span>
			<span class="input-key" v-for="inputKey in keyGroup" :key="inputKey" :class="keyCapWidth(inputKey)">
				{{ inputKey }}
			</span>
		</template>
		<span class="input-mouse" v-if="inputMouse">
			<IconLabel :icon="mouseInputInteractionToIcon(inputMouse)" />
		</span>
		<span class="hint-text" v-if="hasSlotContent">
			<slot></slot>
		</span>
	</div>
</template>

<style lang="scss">
.user-input-label {
	height: 100%;
	margin: 0 8px;
	display: flex;
	align-items: center;
	white-space: nowrap;

	.group-gap {
		width: 4px;
	}

	.input-key,
	.input-mouse {
		& + .input-key,
		& + .input-mouse {
			margin-left: 2px;
		}
	}

	.input-key {
		font-family: "Inconsolata", monospace;
		text-align: center;
		color: var(--color-e-nearwhite);
		border: 1px;
		box-sizing: border-box;
		border-style: solid;
		border-color: var(--color-7-middlegray);
		border-radius: 4px;
		height: 16px;
		line-height: 16px;
	}

	.input-key.width-16 {
		width: 16px;
	}

	.input-key.width-24 {
		width: 24px;
	}

	.input-key.width-32 {
		width: 32px;
	}

	.input-key.width-40 {
		width: 40px;
	}

	.input-key.width-48 {
		width: 48px;
	}

	.input-mouse {
		.bright {
			fill: var(--color-e-nearwhite);
		}

		.dim {
			fill: var(--color-7-middlegray);
		}

		svg {
			vertical-align: top;
		}
	}

	.hint-text {
		margin-left: 4px;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export enum MouseInputInteraction {
	"None" = "None",
	"LMB" = "LMB",
	"RMB" = "RMB",
	"MMB" = "MMB",
	"ScrollUp" = "ScrollUp",
	"ScrollDown" = "ScrollDown",
	"Drag" = "Drag",
	"LMBDrag" = "LMBDrag",
	"RMBDrag" = "RMBDrag",
	"MMBDrag" = "MMBDrag",
}

export default defineComponent({
	components: { IconLabel },
	props: {
		inputKeys: { type: Array, default: () => [] },
		inputMouse: { type: String },
	},
	computed: {
		hasSlotContent(): boolean {
			return Boolean(this.$slots.default);
		},
	},
	methods: {
		keyCapWidth(keyText: string) {
			return `width-${keyText.length * 8 + 8}`;
		},
		mouseInputInteractionToIcon(mouseInputInteraction: MouseInputInteraction) {
			switch (mouseInputInteraction) {
				case MouseInputInteraction.LMB:
					return "MouseHintLMB";
				case MouseInputInteraction.RMB:
					return "MouseHintRMB";
				case MouseInputInteraction.MMB:
					return "MouseHintMMB";
				case MouseInputInteraction.ScrollUp:
					return "MouseHintScrollUp";
				case MouseInputInteraction.ScrollDown:
					return "MouseHintScrollDown";
				case MouseInputInteraction.Drag:
					return "MouseHintDrag";
				case MouseInputInteraction.LMBDrag:
					return "MouseHintLMBDrag";
				case MouseInputInteraction.RMBDrag:
					return "MouseHintRMBDrag";
				case MouseInputInteraction.MMBDrag:
					return "MouseHintMMBDrag";
				default:
				case MouseInputInteraction.None:
					return "MouseHintNone";
			}
		},
	},
	data() {
		return {
			MouseInputInteraction,
		};
	},
});
</script>
