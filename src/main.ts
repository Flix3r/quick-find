import { invoke } from '@tauri-apps/api/core';
import { listen } from "@tauri-apps/api/event";
import { 
    getCurrentWindow, 
    currentMonitor,
    LogicalSize, 
    LogicalPosition, 
} from "@tauri-apps/api/window";

const windowSize = new LogicalSize(200, 300);
const settingsWindowSize = new LogicalSize(800, 500);
const settingsAnimateDuration = 350;

const entrySize = 19; // Needs to be odd
const entryClipText = "...";

const contentSize = entrySize - 2 * entryClipText.length;
const halfSize = (contentSize - 1) / 2;

let settingsOpen = false;

type Entry = {
    string: string,
    selection_index: number,
}

listen('opened', (event) => {
    let entries = event.payload as Entry[];
    let entriesElement = document.getElementById("entries")!;
    entriesElement.style.width = windowSize.width + "px";
    entriesElement.innerHTML = "";

    console.log(entries);

    for (const entry of entries) {
        let entryDiv = document.createElement("div");

        if (entry.selection_index > 0) {
            let preText = document.createElement("span");
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
        entryLetter.innerText = entry.string[entry.selection_index];
        entryDiv.appendChild(entryLetter);
        
        if (entry.string.length >= entry.selection_index) {
            let postText = document.createElement("span");
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
    
    if (settingsOpen) {
        settingsOpen = false;
        openSettings(false);
    }
})

function makeCubicBezier(x1: number, y1: number, x2: number, y2: number) {
  function cubic(a1: number, a2: number, t: number): number {
    return 3 * a1 * (1 - t) ** 2 * t +
           3 * a2 * (1 - t) * t ** 2 +
           t ** 3;
  }

  return function (t: number): number {
    let u = t;
    for (let i = 0; i < 5; i++) {
      const x = cubic(x1, x2, u) - t;
      const dx = 3 * (1 - u) ** 2 * x1 +
                 6 * (1 - u) * u * (x2 - x1) +
                 3 * u ** 2 * (1 - x2);
      if (Math.abs(dx) < 1e-6) break;
      u -= x / dx;
    }
    return cubic(y1, y2, u);
  };
}

const lerp = (a: number, b: number, t: number) => a + t * (b - a);

function openSettings(open: boolean) {
    let window = getCurrentWindow();
    let start: DOMHighResTimeStamp = Number(document.timeline.currentTime);
    let ease = makeCubicBezier(0.25, 1, 0.5, 1);
    let settings = document.getElementById("settings")!;
    let entries = document.getElementById("entries")!;
    if (open) settings.style.display = "flex";
    else entries.style.display = "";

    async function resizeAnimationCallback(timestamp: DOMHighResTimeStamp) {
        if (settingsOpen != open) {
            settings.style.display = settingsOpen ? "flex" : "";
            entries.style.display = settingsOpen ? "none" : "";
            return;
        }

        let progress = Math.min((timestamp - start) / settingsAnimateDuration, 1);
        let progressEased = ease(progress);
        if (!open) progressEased = 1 - progressEased;

        let monitor = await currentMonitor();
        if (!monitor) throw new Error("Monitor not found");

        let newSize = new LogicalSize(
            lerp(windowSize.width, settingsWindowSize.width, progressEased),
            lerp(windowSize.height, settingsWindowSize.height, progressEased)
        );
        let newPos = new LogicalPosition(
            monitor.position.x + (monitor.size.width - newSize.width) / 2,
            monitor.position.y + (monitor.size.height - newSize.height) / 2,
        );

        window.setSize(newSize);
        window.setPosition(newPos);

        settings.style.opacity = String(progressEased);
        entries.style.opacity = String(1 - progressEased);

        if (progress < 1) requestAnimationFrame(resizeAnimationCallback);
        else {
            if (open) entries.style.display = "none";
            else settings.style.display = "";
        }
    }

    resizeAnimationCallback(start);
}

window.addEventListener("keydown", (event) => {
    if (event.key == "Escape") {
        getCurrentWindow().hide();
        return;
    }

    if (event.key.length > 1) return;

    invoke('filter_entries', {inChar: event.key});
});