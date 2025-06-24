export default function computePlayHeadTime(currentTime: number, clips: Array<[number, number]>) {
    if (clips.length === 0) return 0;
    let skippedDuration = clips[0][0];
    for (const [i, clip] of clips.entries()) {
        if (i === 0) continue;
        if (clip[0] > currentTime) {
            break;
        }
        skippedDuration += clip[0] - clips[i - 1][1];
    }
    return currentTime - skippedDuration;
}
