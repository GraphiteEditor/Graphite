<script lang="ts">
export let tooltip: string | undefined = undefined

export let scrollableX: boolean = false
export let scrollableY: boolean = false
</script>

<div
  class="layout-row"
  class:scrollable-x="{scrollableX}"
  class:scrollable-y="{scrollableY}"
  title="{tooltip}"
>
  <slot />
</div>

<style lang="scss">
@import "./layout.scss";
.layout-row {
  display: flex;
  flex-direction: row;
  flex-grow: 1;
}
</style>
