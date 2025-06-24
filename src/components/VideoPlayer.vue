<script setup lang="ts">
import {computed, onBeforeUnmount, ref, watch} from "vue";
import useFileServerAddress from "../composables/fileServerAddress.ts";
import useInput from "../composables/input.ts";

const props = defineProps<{ paused: boolean; clip: [number, number]; visible: boolean }>();
const emit = defineEmits<{
  (e: 'ended'): void;
  (e: 'timeupdate', t: number): void;
}>();

const video = ref<HTMLVideoElement>();
const input = useInput();
const hidden = ref(true);

const shouldUpdateTime = ref(true);

const fileServer = useFileServerAddress();

const inputExtension = computed(() => {
  let i = input.value;
  if (typeof i !== 'string') {
    return 'mkv';
  } else {
    return i.split('.').slice(-1)[0]
  }
});

function updateTime() {
  if (!shouldUpdateTime.value) return;
  let currentTime = video.value?.currentTime;
  if (currentTime !== undefined && !props.paused) {
    emit('timeupdate', currentTime);
  }

  requestAnimationFrame(updateTime);
}

updateTime();

onBeforeUnmount(() => {
  shouldUpdateTime.value = false;
});

watch(video, (video) => {
  if (video && !props.paused) {
    video.play();
  }
  video?.addEventListener('loadeddata', () => {
    hidden.value = false;
  }, {once: true});
  let currentTime = video?.currentTime;
  if (currentTime !== undefined) {
    emit('timeupdate', currentTime);
  }
}, {flush: "post"});

watch(() => props.paused, (paused) => {
  if (paused) {
    video.value?.pause();
    let currentTime = video.value?.currentTime;
    if (currentTime !== undefined) {
      emit('timeupdate', currentTime);
    }
  } else {
    video.value?.play();
  }
});

watch(() => props.visible, (visible) => {
  let v = video.value;
  if (!visible && v) {
    v.currentTime = 0;
  }
});
</script>

<template>
  <video
      ref="video"
      @ended="emit('ended')"
      :class="{'hidden': hidden}"
      :src="fileServer + `input_${clip[0]}-${clip[1]}.${inputExtension}?start=${clip[0]}&end=${clip[1]}`"
      @contextmenu.prevent.stop
      preload="auto"
  />
</template>
