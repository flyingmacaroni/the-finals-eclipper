<script setup lang="ts">
import useProcessVideo from "../composables/processVideo.ts";
import {computed, ref, watch} from "vue";
import ContextMenu from "primevue/contextmenu";
import Button from "primevue/button";
import LogModal from "../components/LogModal.vue";
import TimelineClip from "../components/TimelineClip.vue";
import VideoPlayer from "../components/VideoPlayer.vue";
import PlayHead from "../components/PlayHead.vue";
import _ from "lodash";
import Message from "primevue/message";
import {invoke} from "@tauri-apps/api/core";
import useInput from "../composables/input.ts";
import {useEventListener, useStorage, useThrottle} from "@vueuse/core";
import {save} from "@tauri-apps/plugin-dialog";
import {videoDir} from "@tauri-apps/api/path";
import AddClipDialog from "../components/AddClipDialog.vue";
import {UndoItem} from "../types/undoItem.ts";

const {state} = useProcessVideo();
const clips = ref<Array<[number, number]>>([...state.clips.map((c) => [...c] as [number, number])]);
const input = useInput();
const activeClipIndex = ref(0);
const currentTime = ref(0);
const videoContainer = ref<HTMLDivElement>();
const exportedSuccess = ref(false);
const exportError = ref<string>();
const exporting = ref(false);
const undoHistory = useStorage<Array<UndoItem>>('editHistory', []);
let undoIndex = 0;
const throttledCurrentTime = useThrottle(currentTime, 20, true);
const currentTimeStr = computed(() => {
  let date = new Date(0);
  date.setSeconds(throttledCurrentTime.value);
  return date.toISOString().substring(11, 19);
});
const totalDuration = computed(() => clips.value.reduce((partialSum, [start, end]) => partialSum + (end - start), 0));
const totalDurationStr = computed(() => {
  let date = new Date(0);
  date.setSeconds(totalDuration.value);
  return date.toISOString().substring(11, 19);
});
const processDurationStr = computed(() => {
  let date = new Date(0);
  date.setSeconds(state.elapsed);
  return date.toISOString().substring(11, 19);
});

const logModalOpen = ref(false);

const timeline = ref<HTMLDivElement>();
const paused = ref(true);
const selectedIndex = ref<number | null>(null);
const exportPath = ref<null | string>(null);
const addClipModalOpen = ref(false);

watch(activeClipIndex, (clipIndex) => {
  if (paused.value) {
    handleUpdateTime(clipIndex, 0);
  }
});

watch(currentTime, _.throttle((currentTime: number) => {
  if (paused.value) return;

  scrollToTime(currentTime);
}, 20, {trailing: true}));

useEventListener('keydown', (event) => {
  if (event.code === 'KeyX') {
    removeClip();
  }
  if (event.code === 'KeyZ' && event.shiftKey && event.ctrlKey) {
    redo();
  } else if (event.code === 'KeyZ' && event.ctrlKey) {
    undo();
  }
  if (event.code === 'Space') {
    handleTogglePause();
  }
  if (event.code === 'KeyA' && event.shiftKey && !event.ctrlKey) {
    addClipModalOpen.value = true;
  }
});

function removeClip(index?: number) {
  let i = typeof index === 'number' ? index : activeClipIndex.value;
  let clip = _.cloneDeep(clips.value[i]);
  clips.value.splice(i, 1);
  undoHistory.value.splice(undoIndex);
  undoHistory.value.push({type: 'delete', clip});
  undoIndex += 1;
}

// undo deleting
function undo() {
  if (undoIndex === 0) {
    return;
  }

  undoIndex = Math.max(0, undoIndex - 1);
  let undoItem = undoHistory.value[undoIndex];

  if (undoItem.type === 'delete') {
    let clip = undoItem.clip;
    let insertIndex = clips.value.findIndex((c) => c[1] > clip[0]);
    clips.value.splice(insertIndex, 0, clip);
  }

  if (undoItem.type === 'change') {
    // undoing to old
    clips.value = _.cloneDeep(undoItem.clips.old);
  }
}

// redo deleting
function redo() {
  if (undoIndex === undoHistory.value.length) {
    return;
  }
  let undoItem = undoHistory.value[undoIndex];

  if (undoItem.type === 'delete') {
    let clip = undoItem.clip;
    let clipIndex = clips.value.findIndex(([start, end]) => (start === clip[0] && end === clip[1]));

    if (clipIndex !== -1) {
      clips.value.splice(clipIndex, 1);
      undoIndex = Math.min(undoHistory.value.length, undoIndex + 1);
    }
  }
  if (undoItem.type === 'change') {
    clips.value = _.cloneDeep(undoItem.clips.new);
    undoIndex = Math.min(undoHistory.value.length, undoIndex + 1);
  }
}

function handleExport() {
  save({
    defaultPath: input.value?.split('.').slice(0, -1).join('.').concat('.eclipper_trimmed.').concat(input.value?.split('.').slice(-1)[0]),
    filters: [{
      name: 'Video',
      extensions: ['mp4', 'mkv', 'webm']
    }]
  }).then((path) => {
    if (path === input.value) {
      exportError.value = 'Export path is the same as the input path.'
      return;
    }
    if (typeof path === 'string') {
      exportedSuccess.value = false;
      exportError.value = undefined;
      exporting.value = true;
      exportPath.value = path;
      invoke('write_clips', {clips: clips.value, path, keyframes: state.keyframes}).then(() => {
        exportedSuccess.value = true;
      }).catch((e) => {
        exportError.value = e.message;
      }).finally(() => exporting.value = false);
    }
  }).catch((e) => {
    exportError.value = e.message;
  });
}

function scrollToTime(time: number) {
  if (timeline.value) {
    let width = timeline.value?.clientWidth;
    let offset = timeline.value?.scrollLeft;
    let playHeadPos = time * 16;
    if (playHeadPos < offset + 16) timeline.value?.scrollTo({left: playHeadPos - 16});
    if (playHeadPos + 16 > offset + width) timeline.value?.scrollTo({left: playHeadPos - width + 64});
  }
}

async function handleSaveClip() {
  if (selectedIndex.value === null) return;
  const videoDirPath = await videoDir();
  save({
    defaultPath: videoDirPath + 'clip.'.concat(input.value?.split('.').slice(-1)[0] ?? 'mp4'),
    filters: [{
      name: 'Video',
      extensions: ['mp4', 'mkv', 'webm'],
    }]
  }).then((path) => {
    console.log(path);
    if (typeof path === 'string' && selectedIndex.value !== null) {
      exportedSuccess.value = false;
      exportError.value = undefined;
      exporting.value = true;
      exportPath.value = path;
      let clip = _.cloneDeep(clips.value[selectedIndex.value]);
      console.log(clip);
      invoke('write_clips', {clips: [clip], path, keyframes: state.keyframes}).then(() => {
        exportedSuccess.value = true;
      }).catch((e) => {
        exportError.value = e.message;
      }).finally(() => exporting.value = false);
    }
  });
}

function playNextClip() {
  activeClipIndex.value = Math.min(activeClipIndex.value + 1, clips.value.length - 1);
}

function playPreviousClip() {
  activeClipIndex.value = Math.max(activeClipIndex.value - 1, 0);
}

function jumpToClip(clipIndex: number) {
  activeClipIndex.value = clipIndex;
}

function handleClipEnded() {
  if (activeClipIndex.value === clips.value.length - 1) {
    paused.value = true;
    setTimeout(() => {
      currentTime.value = totalDuration.value;
    });
  }
  activeClipIndex.value = Math.min(activeClipIndex.value + 1, clips.value.length - 1);
}

function handleUpdateTime(clipIndex: number, time: number) {
  if (activeClipIndex.value === clipIndex) {
    currentTime.value = time + clips.value.slice(0, clipIndex).reduce((partialSum, [start, end]) => partialSum + (end - start), 0)
  }
}

function handleTogglePause() {
  // restart if at end
  if (paused.value && currentTime.value >= totalDuration.value - 0.1) {
    activeClipIndex.value = 0;
    paused.value = false;
    return;
  }
  paused.value = !paused.value;
}

const menu = ref();

function handleClipRightClick(index: number, event: MouseEvent) {
  activeClipIndex.value = index;
  selectedIndex.value = index;
  (menu.value as any)?.show(event);
}

function handleAddClip(clip: [number, number]) {
  let old = _.cloneDeep(clips.value);
  let overlappingClips = [clip];
  let overlappingClip = clips.value.findIndex(([s, e]) => (clip[0] >= s && clip[0] <= e) || (clip[1] >= s && clip[1] <= e));
  while (overlappingClip !== -1) {
    overlappingClips.push(_.cloneDeep(clips.value[overlappingClip]));
    clips.value.splice(overlappingClip, 1);
    overlappingClip = clips.value.findIndex(([s, e]) => (clip[0] >= s && clip[0] <= e) || (clip[1] >= s && clip[1] <= e));
  }
  clip = [Math.min(...overlappingClips.map(([s,]) => s)), Math.max(...overlappingClips.map(([, e]) => e))];
  let insertIndex = clips.value.findIndex(([_, end]) => clip[0] <= end);
  if (insertIndex === -1) {
    clips.value.push(clip);
    activeClipIndex.value = clips.value.length - 1;
    let time = clips.value.slice(0, activeClipIndex.value).reduce((partialSum, [start, end]) => partialSum + (end - start), 0)
    scrollToTime(time + (clip[1] - clip[0]));
    scrollToTime(time);
    undoHistory.value.push({type: 'change', clips: {old, new: _.cloneDeep(clips.value)}});
    undoIndex += 1;
    return;
  }
  clips.value.splice(insertIndex, 0, clip);
  activeClipIndex.value = insertIndex;
  let time = clips.value.slice(0, activeClipIndex.value).reduce((partialSum, [start, end]) => partialSum + (end - start), 0);
  undoHistory.value.push({type: 'change', clips: {old, new: _.cloneDeep(clips.value)}});
  undoIndex += 1;
  scrollToTime(time + (clip[1] - clip[0]));
  scrollToTime(time);
}
</script>

<template>
  <div class="h-screen w-screen flex flex-col max-h-screen gap-1 p-1">
    <div class="flex justify-between">
      <Button @click="logModalOpen = true" severity="secondary" label="Logs" size="small" icon="pi pi-align-left" text/>
      <Button label="Export" size="small" text @click="handleExport" :loading="exporting"/>
    </div>
    <Message v-if="state.elapsed > 1" class="m-0" severity="success">
      Video processed in {{ processDurationStr }}
    </Message>
    <Message severity="success" class="m-0" v-if="exportedSuccess">
      Clip(s) exported successfully to '{{ exportPath }}'
    </Message>
    <Message severity="error" class="m-0" v-if="exportError !== undefined">{{ exportError }}</Message>
    <Message severity="info" class="m-0">Press 'X' to delete active clip. Press 'Ctrl+Z' to undo. Press 'Shift+A' to add
      a clip.
    </Message>
    <AddClipDialog
        v-model:open="addClipModalOpen" v-if="state.keyframes.length > 0 && state.inputDuration"
        :keyframes="state.keyframes" :vide-duration="state.inputDuration" @add-clip="handleAddClip"
    />
    <div
        class="video-panel grow min-h-0 flex flex-col no-header bg-slate-800 rounded-lg overflow-hidden border border-solid border-gray-700">
      <div class="relative min-h-0 grow bg-black" ref="videoContainer" @click="handleTogglePause">
        <template v-for="(clip, index) in clips">
          <VideoPlayer
              v-if="index - activeClipIndex >= 0 && index - activeClipIndex < 3"
              class="w-full h-full"
              :class="{'hidden': activeClipIndex !== index}"
              :clip="clip"
              :visible="activeClipIndex === index"
              :paused="paused || activeClipIndex !== index"
              @timeupdate="handleUpdateTime(index, $event)"
              @ended="handleClipEnded"
              playsinline
          />
        </template>
        <!--suppress JSUnusedLocalSymbols -->
      </div>
      <!--media controls-->
      <div class="flex justify-between items-center h-6 my-2 py-1 relative">
        <div class="px-4">
          {{ paused ? 'Paused' : '' }}
        </div>
        <div class="absolute inset-0 flex items-center justify-center">
          <Button
              class="!h-6" icon="pi pi-step-backward" severity="secondary" size="small" text
              @click="playPreviousClip"
          />
          <Button
              class="!h-6" :icon="`pi pi-${paused ? 'play' : 'pause'}`" severity="secondary" size="small" text
              @keydown.prevent
              @click="handleTogglePause"
          />
          <Button class="!h-6" icon="pi pi-step-forward" severity="secondary" size="small" text @click="playNextClip"/>
        </div>
        <div class="flex items-center px-4 font-mono">
          {{ currentTimeStr }} / {{ totalDurationStr }}
        </div>
      </div>
      <!--end media controls-->
    </div>
    <div class="bg-slate-800 overflow-hidden rounded-lg shrink-0  border border-solid border-gray-700">
      <div
          v-if="clips.length > 0"
          ref="timeline"
          class="timeline shrink-0 flex flex-row max-w-screen overflow-x-auto overflow-y-hidden p-1 relative"
      >
        <TimelineClip
            v-for="(clip, index) in clips" :clip="clip" :key="clip[0]" :label="`#${index + 1}`"
            :active="activeClipIndex === index"
            @contextmenu.stop.prevent="handleClipRightClick(index, $event)"
            @timeupdate="handleUpdateTime(index, $event)"
            @click="jumpToClip(index)"
        />
        <ContextMenu
            ref="menu"
            :model="[{icon: 'pi pi-trash', label: 'Delete Clip', shortcut: 'X', action: removeClip}, {icon: 'pi pi-save', label: 'Export Clip', action: handleSaveClip}]"
        >
          <template #item="{ item, props }">
            <a v-ripple class="flex align-items-center" @click="item.action(selectedIndex)" v-bind="props.action">
              <span :class="item.icon"/>
              <span class="ml-2">{{ item.label }}</span>
              <span v-if="item.shortcut"
                    class="ml-auto border border-solid rounded surface-border surface-100 text-xs px-1">
                {{ item.shortcut }}
              </span>
              <i v-if="item.items" class="pi pi-angle-right ml-auto"></i>
            </a>
          </template>
        </ContextMenu>
        <PlayHead :current-time="currentTime" class="pl-1"/>
      </div>
      <div v-else>
        Missing clips {{ clips }}
      </div>
    </div>
    <LogModal v-model:open="logModalOpen"/>
  </div>
</template>

<style scoped>
/* width */
::-webkit-scrollbar {
  height: 0.75rem;
}

/* Track */
::-webkit-scrollbar-track {
  background: var(--surface-0);
}

/* Handle */
::-webkit-scrollbar-thumb {
  background: var(--surface-300);
  border-radius: 0.25rem;
}

/* Handle on hover */
::-webkit-scrollbar-thumb:hover {
  background: var(--surface-400);
}

.surface-100 {
  background-color: var(--surface-300);
}

.surface-border {
  border-color: var(--surface-border);
}
</style>
