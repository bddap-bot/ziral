import {spawn, execFileSync} from 'node:child_process';
import {createServer} from 'node:http';
import {mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {extname, join} from 'node:path';

const MARK = 30;
const END = MARK + 17;
const ATTEMPTS = 3;
const SIZE = {width: 1280, height: 720};

const [page = 'web/dist', kept] = process.argv.slice(2);
const scratch = kept ?? mkdtempSync(join(tmpdir(), 'ziral-bench-'));
const sleep = seconds => new Promise(resolve => setTimeout(resolve, seconds * 1000));

function ziral(...args) {
    return execFileSync('cargo', ['run', '--release', '--quiet', '--', ...args], {encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit']});
}

async function serve(directory) {
    const types = {'.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm'};
    const server = createServer((request, response) => {
        const path = join(directory, new URL(request.url, 'http://page').pathname.replace(/\/$/, '/index.html'));
        try {
            const body = readFileSync(path);
            response.writeHead(200, {'content-type': types[extname(path)] ?? 'application/octet-stream'});
            response.end(body);
        } catch {
            response.writeHead(404);
            response.end();
        }
    });
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    return server;
}

function browse(host) {
    const chrome = spawn('chromium', [
        '--headless=new', '--remote-debugging-pipe', '--no-first-run', '--no-default-browser-check',
        `--user-data-dir=${join(scratch, 'profile')}`,
        `--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE ${host}`,
        '--use-gl=angle', '--use-angle=gl-egl', '--ignore-gpu-blocklist',
        '--disable-frame-rate-limit', '--disable-gpu-vsync', '--js-flags=--no-wasm-dynamic-tiering',
        'about:blank',
    ], {stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe']});
    let id = 0;
    let buffer = '';
    const pending = new Map();
    chrome.stdio[4].on('data', chunk => {
        buffer += chunk;
        for (let end; (end = buffer.indexOf('\0')) >= 0; buffer = buffer.slice(end + 1)) {
            const message = JSON.parse(buffer.slice(0, end));
            const waiting = pending.get(message.id);
            if (!waiting) continue;
            pending.delete(message.id);
            if (message.error) waiting.reject(new Error(JSON.stringify(message.error)));
            else waiting.resolve(message.result);
        }
    });
    const send = (method, params = {}, sessionId) => new Promise((resolve, reject) => {
        pending.set(++id, {resolve, reject});
        chrome.stdio[3].write(JSON.stringify({id, method, params, sessionId}) + '\0');
    });
    return {chrome, send};
}

async function record(url) {
    const {chrome, send} = browse(new URL(url).hostname);
    try {
        const {targetId} = await send('Target.createTarget', {url: 'about:blank'});
        const {sessionId} = await send('Target.attachToTarget', {targetId, flatten: true});
        const evaluate = async expression => (await send('Runtime.evaluate', {expression, returnByValue: true, awaitPromise: true}, sessionId)).result.value;
        await send('Emulation.setDeviceMetricsOverride', {...SIZE, deviceScaleFactor: 1, mobile: false}, sessionId);
        await send('Page.navigate', {url: `${url}#bench`}, sessionId);
        const chunks = `new Promise((resolve, reject) => {
            const opening = indexedDB.open('ziral-records', 1);
            opening.onupgradeneeded = () => opening.result.createObjectStore('records');
            opening.onerror = () => reject(opening.error);
            opening.onsuccess = () => {
                const store = opening.result.transaction('records', 'readonly').objectStore('records');
                const keys = store.getAllKeys();
                const values = store.getAll();
                values.onsuccess = () => {
                    const saved = new Map(keys.result.map((key, index) => [key, values.result[index]]));
                    for (const key of Object.keys(localStorage).filter(key => key.startsWith('ziral-record-'))) {
                        saved.set(key, localStorage.getItem(key));
                    }
                    resolve([...saved.values()].map(text => JSON.parse(text)));
                };
            };
        })`;
        for (let waited = 0; !(await evaluate(chunks)).length; waited++) {
            if (waited > 600) throw new Error(`${url} recorded nothing`);
            await sleep(0.5);
        }
        const press = async (key, code, windowsVirtualKeyCode) => {
            for (const type of ['keyDown', 'keyUp']) {
                await send('Input.dispatchKeyEvent', {type, key, code, windowsVirtualKeyCode}, sessionId);
            }
        };
        await evaluate(`document.querySelector('canvas').focus()`);
        await press(' ', 'Space', 32);
        await sleep(MARK);
        await press('m', 'KeyM', 77);
        await sleep(END - MARK);
        if (kept) {
            const {data} = await send('Page.captureScreenshot', {format: 'png'}, sessionId);
            writeFileSync(join(scratch, 'web.png'), Buffer.from(data, 'base64'));
        }
        await evaluate(`dispatchEvent(new Event('pagehide'))`);
        const saved = await evaluate(chunks);
        const sessions = new Set(saved.map(chunk => chunk.session));
        if (sessions.size !== 1) throw new Error(`expected one recorded session, found ${sessions.size}`);
        saved.sort((a, b) => a.start - b.start);
        return {build: saved[0].build, seed: saved[0].seed, inputs: saved.flatMap(chunk => chunk.inputs)};
    } finally {
        const exited = new Promise(resolve => chrome.once('exit', resolve));
        chrome.kill();
        await exited;
        rmSync(join(scratch, 'profile'), {recursive: true, force: true});
    }
}

function steady(target, attempt, path) {
    const {frames, marks} = JSON.parse(ziral('--analyze', path));
    if (marks.length !== 1) throw new Error(`${target}: expected one mark, found ${marks.length}`);
    const window = marks[0].frames;
    console.log(`${target} window ${attempt}: ${window.count} frames, p50 ${window.p50_ms} ms, p90 ${window.p90_ms} ms, p99 ${window.p99_ms} ms, max ${window.max_ms} ms, ${window.over_budget} over budget (whole run: ${frames.count} frames, max ${frames.max_ms} ms)`);
    return window.over_budget === 0;
}

async function holds(target, measure) {
    for (let attempt = 1; attempt <= ATTEMPTS; attempt++) {
        const path = join(scratch, `${target}-${attempt}.json`);
        await measure(path);
        if (steady(target, attempt, path)) return true;
    }
    return false;
}

try {
    const native = await holds('native', async path => ziral('--bench', path, String(MARK), String(END)));
    let url = page;
    let server;
    if (!/^https?:/.test(page)) {
        server = await serve(page);
        url = `http://127.0.0.1:${server.address().port}/`;
    }
    let web;
    try {
        web = await holds('web', async path => writeFileSync(path, JSON.stringify(await record(url))));
    } finally {
        server?.close();
    }
    process.exitCode = native && web ? 0 : 1;
} finally {
    if (!kept) rmSync(scratch, {recursive: true, force: true});
}
