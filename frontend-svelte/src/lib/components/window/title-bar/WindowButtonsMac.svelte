<script lang="ts">
  import LayoutRow from "$lib/components/layout/LayoutRow.svelte";

  export let maximized = false;
</script>

<template>
  <LayoutRow class="window-buttons-mac">
    <div class="close" title="Close" />
    <div class="minimize" title="Minimize" />
    <div class="zoom" title="Zoom" />
  </LayoutRow>
</template>

<style lang="scss" global>
  .window-buttons-mac {
    flex: 0 0 auto;
    align-items: center;
    margin: 0 8px;

    div {
      flex: 0 0 auto;
      align-items: center;
      width: 11px;
      height: 11px;
      border-radius: 50%;

      & + div {
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
