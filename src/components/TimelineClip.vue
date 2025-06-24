<script setup lang="ts">
import {computed} from "vue";

const props = defineProps<{clip: [number, number], label: string, active: boolean}>();
const emit = defineEmits<{
  (e: 'timeupdate', t: number): void;
  (e: 'contextmenu', v: MouseEvent): void;
}>();

const duration = computed(() => props.clip[1] - props.clip[0]);

const timeRangeStr = computed(() => {
  let date = new Date(0);
  date.setSeconds(props.clip[0]); // specify value for SECONDS here
  let startTimeString = date.toISOString().substring(11, 19);
  date = new Date(0);
  date.setSeconds(props.clip[1]);
  let endTimeString = date.toISOString().substring(11, 19);
  return `${startTimeString} - ${endTimeString}`;
});

function handleClick(e: MouseEvent) {
  if (!props.active) return;
  if (e.button !== 0) return;
  let target = e.target as HTMLDivElement;
  let rect = target.getBoundingClientRect();
  let x = e.clientX - rect.left; //x position within the element.
  let seconds = x / 16;
  let videoPlayer = document.querySelector('video:not(.hidden)');
  if (videoPlayer) {
    if ('currentTime' in videoPlayer) {
      emit('timeupdate', seconds);
      videoPlayer.currentTime = seconds;
    }
  }
}
</script>

<template>
  <div tabindex="0" ref="timelineClip" class="shrink-0 cursor-default rounded-lg h-16 min-h-16 py-1 px-2 border border-solid border-blue-500 truncate" :style="`width: ${duration}rem;`" :class="active ? 'bg-slate-900' : 'bg-blue-900'" @mousedown="handleClick" @contextmenu="emit('contextmenu', $event)">
    {{props.label}} <br>
    <small>
    {{timeRangeStr}}
    </small>
  </div>
</template>
