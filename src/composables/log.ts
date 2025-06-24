import {listen} from "@tauri-apps/api/event";
import {ref} from "vue";
// @ts-ignore
import {parse} from "ansicolor";

const log = ref<ParsedSpan[]>([]);

listen<string>('log', (event) => {
    addLog(event.payload);
}).then();

export default function useLog() {
    return log.value;
}

export function addLog(text: string) {
    let newLogs: ParsedSpan[] = parse(text).spans.map((span: ParsedSpan) => {
        let css = span.css;
        if (span.color?.dim) {
            css = 'color: gray;'
        }
        return {...span, css}
    });
    log.value.push(...newLogs);
}

export type ParsedSpan = {
    text: string;
    css: string;
    italic?: boolean;
    bold?: boolean;
    color?: ParsedColor;
    bgColor?: ParsedColor;
}

export type ParsedColor = {
    name?:   string;
    bright?: boolean;
    dim?:    boolean;
};
