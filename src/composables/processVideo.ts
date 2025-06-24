import {reactive} from "vue";
import {invoke} from "@tauri-apps/api/core";
import useInput from "./input.ts";
import useThreads from "./threads.ts";
import {addLog} from "./log.ts";
import {listen} from "@tauri-apps/api/event";

const state = reactive({
    processing: false,
    includeAssists: true,
    includeSpectating: false,
    elimClipDuration: 4.0,
    hwAccel: true,
    progress: 0,
    speed: 0,
    elapsed: 0,
    clips: [] as Array<[number, number]>,
    keyframes: [] as Array<number>,
    inputDuration: null as number | null,
});

export default function useProcessVideo() {
    const input = useInput();
    const threads = useThreads();

    if (state.clips.length === 0) {
        let clips = window.localStorage.getItem('clips');
        let keyframes = window.localStorage.getItem('keyframes');
        let inputDuration = window.localStorage.getItem('inputDuration');
        if (clips !== null) {
            state.clips = JSON.parse(clips);
        }
        if (keyframes !== null) {
            state.keyframes = JSON.parse(keyframes);
        }
        if (inputDuration !== null) {
            state.inputDuration = Number(inputDuration);
        }
    }

    return {
        state,
        async start() {
            // cannot start processing if already processing
            if (state.processing) {
                return;
            }
            state.progress = 0;
            state.speed = 0;
            state.processing = true;
            state.elapsed = 0;
            let args = {
                input: input.value,
                threads: threads.value,
                includeAssists: state.includeAssists,
                includeSpectating: state.includeSpectating,
                elimClipDuration: state.elimClipDuration,
                hwAccel: state.hwAccel,
            };
            console.log(args);
            let instant = new Date();
            let result = await invoke<{
                clips: Array<[number, number]>,
                keyframes: Array<number>,
                inputDuration: number,
            }>('process', args).catch((e) => addLog(e + '\n')).finally(() => {
                state.processing = false;
            }).then((result) => {
                console.log(result);
                if (result) {
                    console.log(result.clips);
                    console.log(result.keyframes);
                    // @ts-ignore
                    state.elapsed = ((new Date()) - instant) / 1000;
                    state.clips = result.clips;
                    state.keyframes = result.keyframes;
                    state.inputDuration = result.inputDuration ?? result.keyframes.slice(-1)[0];
                    window.localStorage.setItem('clips', JSON.stringify(result.clips));
                    window.localStorage.setItem('keyframes', JSON.stringify(result.keyframes));
                    window.localStorage.setItem('inputDuration', state.inputDuration.toString());
                }
                return result;
            });
            console.log(result);
        }
    }
}

listen<{ progress: number, speed: number }>('progress', (event) => {
    state.progress = event.payload.progress;
    state.speed = event.payload.speed;
}).then();
