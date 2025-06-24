<script setup lang="ts">
 defineProps<{
  currentTime: number;
}>();
</script>

<template>
  <div ref="el" class="border-r-2 border-y-0 border-l-0 border-solid border-red-500 absolute top-0 bottom-0" :style="{left: currentTime + 'rem'}" />
</template>