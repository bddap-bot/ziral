const module = await WebAssembly.compileStreaming(fetch(new URL("./ziral_bg.wasm", import.meta.url)));
postMessage(module);
