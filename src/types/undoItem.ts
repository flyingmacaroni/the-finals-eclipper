type Delete = {
    type: 'delete',
    clip: [number, number],
};

type ChangeClips = {
    type: 'change',
    clips: {old: Array<[number, number]>, new: Array<[number, number]>},
}

export type UndoItem = Delete | ChangeClips;
