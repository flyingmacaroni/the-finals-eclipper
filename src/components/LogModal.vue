<script setup lang="ts">
import useLog from "../composables/log.ts";
import Dialog from "primevue/dialog";

defineProps<{open: boolean}>();
const emit = defineEmits<{
  (e: 'update:open', open: boolean): void;
}>();

const log = useLog();
</script>

<template>
  <Dialog :visible="open" content-class="h-full flex flex-col" @update:visible="emit('update:open', $event)">
    <div class="grow shrink bg-black whitespace-pre-wrap text-white overflow-y-scroll flex flex-col-reverse">
      <div>
      <span v-for="l in log" :style="l.css">
        {{ l.text }}
      </span>
      </div>
    </div>
  </Dialog>
</template>
