const mixer = `registerProcessor("mixer", class extends AudioWorkletProcessor {
    clips = [];
    voices = [];
    constructor() {
        super();
        this.port.onmessage = ({ data }) => {
            if (data.samples) this.clips[data.clip] = data.samples;
            else if (this.clips[data[0]]) this.voices.push({ samples: this.clips[data[0]], gain: data[1], speed: data[2], at: 0 });
        };
    }
    process(_, [[out]]) {
        for (let v = this.voices.length - 1; v >= 0; v--) {
            const voice = this.voices[v];
            const end = Math.min(out.length, Math.ceil((voice.samples.length - voice.at) / voice.speed));
            for (let i = 0; i < end; i++) {
                const at = voice.at + i * voice.speed;
                const whole = Math.floor(at);
                const a = voice.samples[whole];
                const b = voice.samples[whole + 1] ?? 0;
                out[i] += voice.gain * (a + (b - a) * (at - whole));
            }
            voice.at += end * voice.speed;
            if (voice.at >= voice.samples.length) this.voices[v] = this.voices[this.voices.length - 1], this.voices.pop();
        }
        return true;
    }
});`;
const inputEvents = ["pointerdown", "pointerup", "keydown", "touchend"];
let context;
let port;
let ready;
let clips = 0;
function resume() {
    context.resume().then(() => {
        if (context.state === "running")
            for (const type of inputEvents)
                globalThis.removeEventListener(type, resume, { capture: true });
    });
}
function open(rate) {
    context = new AudioContext({ sampleRate: rate });
    for (const type of inputEvents)
        globalThis.addEventListener(type, resume, { capture: true });
    const url = URL.createObjectURL(new Blob([mixer], { type: "text/javascript" }));
    ready = context.audioWorklet.addModule(url).then(() => {
        URL.revokeObjectURL(url);
        const node = new AudioWorkletNode(context, "mixer", { numberOfInputs: 0, outputChannelCount: [1] });
        node.connect(context.destination);
        return port = node.port;
    });
}
export function add_clip(samples, rate) {
    if (!context) open(rate);
    const clip = clips++;
    const copy = samples.slice();
    ready.then(port => port.postMessage({ clip, samples: copy }, [copy.buffer]));
    return clip;
}
export function play_clip(clip, gain, speed) {
    if (context.state === "running") port?.postMessage([clip, gain, speed]);
}
