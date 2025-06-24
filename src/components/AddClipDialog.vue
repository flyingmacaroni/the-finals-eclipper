<script setup lang="ts">
import Dialog from "primevue/dialog";
import InputNumber from "primevue/inputnumber";
import {ref} from "vue";
import Button from "primevue/button";
import Message from "primevue/message";

const props = defineProps<{
  open: boolean;
  keyframes: Array<number>;
  videDuration: number;
}>();

const emit = defineEmits<{
  (e: 'update:open', open: boolean): void;
  (e: 'addClip', clip: [number, number]): void;
}>();

const startH = ref(0);
const startM = ref(0);
const startS = ref(0);
const duration = ref<number>(5);
const errorMessage = ref<string>();

function handleAddClip() {
  errorMessage.value = undefined;
  let start = (startH.value * 60 * 60) + (startM.value * 60) + startS.value;
  let end = start + duration.value;
  let startIndex = props.keyframes.findIndex((ts) => ts >= start);
  if (startIndex === -1 && start !== 0) {
    errorMessage.value = 'Invalid clip start time';
    return;
  }
  let actualStart;
  if (start === 0) {
    actualStart = start;
  } else {
    actualStart = props.keyframes[startIndex];
  }
  if (actualStart > start + 0.5) {
    actualStart = props.keyframes[Math.max(startIndex - 1, 0)];
  }
  let endIndex = props.keyframes.findIndex((ts) => ts >= end);
  let actualEnd;
  if (endIndex === -1) {
    actualEnd = props.videDuration;
  } else {
    actualEnd = props.keyframes[endIndex];
  }

  emit('addClip', [actualStart, actualEnd]);
  emit('update:open', false);
}
</script>

<template>
  <Dialog :visible="open" @update:visible="emit('update:open', $event)" header="Add Clip">
    <div class="flex flex-col gap-3">
      <Message v-if="errorMessage !== undefined" severity="error">{{errorMessage}}</Message>
      <div class="flex flex-col gap-2">
        <span class="font-bold">Clip Start</span>
        <div class="flex flex-row gap-3 items-center">

            <InputNumber :min="0" :max="255" v-model="startH" input-class="w-16" />
:
            <InputNumber :min="0" :max="59" v-model="startM" input-class="w-16" />
:
            <InputNumber :min="0" :max="59" v-model="startS" input-class="w-16"/>
        </div>
      </div>
      <div class="flex flex-col gap-2">
        <span class="font-bold">Clip Duration (Seconds)</span>
        <InputNumber v-model="duration" :min="1" showIcon icon="pi pi-clock" />
      </div>
      <Button label="Add Clip" @click="handleAddClip" />
    </div>
  </Dialog>
</template>
