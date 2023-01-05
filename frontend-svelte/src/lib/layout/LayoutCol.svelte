<script lang="ts">
export let tooltip: string | undefined = undefined

export let scrollableX: boolean = false
export let scrollableY: boolean = false
</script>

<div
  class="layout-col"
  class:scrollable-x="{scrollableX}"
  class:scrollable-y="{scrollableY}"
  title="{tooltip}"
>
  <slot />
</div>

<style lang="scss">
@import "./_layout.scss";
.layout-col {
  display: flex;
  flex-direction: column;
  flex-grow: 1;
}
</style>
