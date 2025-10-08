import { invoke } from '@tauri-apps/api/core';
import { listen } from "@tauri-apps/api/event";

const entrySize = 19; // Needs to be odd
const entryClipText = "...";

const contentSize = entrySize - 2 * entryClipText.length;
const halfSize = (contentSize - 1) / 2;

type Entry = {
    string: string,
    selection_index: number,
}

listen('opened', (event) => {
    let entries = event.payload as Entry[];
    let entriesElement = document.getElementById("entries")!;
    entriesElement.innerHTML = "";

    console.log(entries);

    for (const entry of entries) {
        let entryDiv = document.createElement("div");
        entryDiv.className = "entry";

        if (entry.selection_index > 0) {
            let preText = document.createElement("span");
            preText.className = "pre";

            let start = Math.max(0, Math.min(
                entry.selection_index - halfSize, 
                entry.string.length - entrySize + entryClipText.length
            ));
            if (start > 0 && start <= entryClipText.length) {
                start = 0;
            }
            preText.innerText = (start > 0 ? entryClipText : "")
                + entry.string.slice(start, entry.selection_index);
            entryDiv.appendChild(preText);
        }
        
        let entryLetter = document.createElement("em");
        entryLetter.className = "current";

        entryLetter.innerText = entry.string[entry.selection_index];
        entryDiv.appendChild(entryLetter);
        
        if (entry.string.length >= entry.selection_index) {
            let postText = document.createElement("span");
            postText.className = "post";

            let end = Math.min(entry.string.length, Math.max(
                entry.selection_index + halfSize + 1,
                entrySize - entryClipText.length
            ));
            if (
                end < entry.string.length 
                && end >= entry.string.length - entryClipText.length
            ) {
                end = entry.string.length;
            }
            postText.innerText = entry.string.slice(entry.selection_index + 1, end) 
                + (end < entry.string.length ? entryClipText : "");
            entryDiv.appendChild(postText);
        }

        entriesElement.appendChild(entryDiv);
    }
});

listen('custom-css', (event) => {
    document.getElementById('custom-css')!.innerHTML = event.payload as string;
});

window.addEventListener("keydown", (event) => {
    if (event.key == "Escape") {
        invoke('close');
        return;
    }

    if (event.ctrlKey && event.key == ".") {
        invoke('open_config');
        return;
    }

    if (event.key.length > 1) return;

    invoke('filter_entries', {inChar: event.key});
});