import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {getPriority, tmpdir} from 'node:os';
import {extname, join} from 'node:path';

const MARKS = [30, 60];
const WINDOW = 30;
const END = MARKS.at(-1) + WINDOW / 2 + 2;
const SIZE = {width: 1280, height: 720};
const PATIENCE = 30 * 60;

const [page = 'web/dist', kept] = process.argv.slice(2);
const scratch = kept ?? mkdtempSync(join(tmpdir(), 'ziral-bench-'));
const sleep = seconds => new Promise(resolve => setTimeout(resolve, seconds * 1000));

const cpus = new Set(readFileSync('/proc/self/status', 'utf8').match(/^Cpus_allowed_list:\s*(.+)$/m)[1].split(',').flatMap(range => {
    const [first, last = first] = range.split('-').map(Number);
    return Array.from({length: last - first + 1}, (_, offset) => first + offset);
}));
const BUSY = cpus.size / 4;
const lowered = getPriority() > 0;

const spent = new Map();
function own() {
    const pids = [process.pid];
    while (pids.length) {
        const pid = pids.pop();
        let fields;
        try {
            const stat = readFileSync(`/proc/${pid}/stat`, 'utf8');
            fields = stat.slice(stat.lastIndexOf(')') + 2).split(' ');
        } catch {
            continue;
        }
        spent.set(`${pid}@${fields[19]}`, Number(fields[11]) + Number(fields[12]));
        let tasks = [];
        try { tasks = readdirSync(`/proc/${pid}/task`); } catch {}
        for (const task of tasks) {
            try { pids.push(...readFileSync(`/proc/${pid}/task/${task}/children`, 'utf8').split(' ').filter(Boolean)); } catch {}
        }
    }
    let ticks = 0;
    for (const value of spent.values()) ticks += value;
    return ticks;
}

const samples = [];
setInterval(() => {
    let spare = own();
    for (const line of readFileSync('/proc/stat', 'utf8').split('\n')) {
        const [name, , nice, , idle, iowait] = line.split(' ');
        if (/^cpu\d+$/.test(name) && cpus.has(Number(name.slice(3)))) spare += (lowered ? 0 : Number(nice)) + Number(idle) + Number(iowait);
    }
    samples.push([performance.now(), spare]);
}, 100).unref();

function others(from, to) {
    let most = 0;
    for (let index = 1; index < samples.length; index++) {
        const [[start, before], [end, after]] = [samples[index - 1], samples[index]];
        if (end > from && start < to) most = Math.max(most, cpus.size - (after - before) * 10 / (end - start));
    }
    return most;
}

async function settle(deadline) {
    while (performance.now() < deadline) {
        const from = performance.now();
        await sleep(5);
        if (others(from, performance.now()) <= BUSY) return;
    }
}

function ziral(...args) {
    return new Promise((resolve, reject) => {
        const child = spawn('cargo', ['run', '--release', '--quiet', '--', ...args], {stdio: ['ignore', 'pipe', 'inherit']});
        let output = '';
        child.stdout.setEncoding('utf8').on('data', chunk => output += chunk);
        child.once('error', reject);
        child.once('close', code => code === 0 ? resolve(output) : reject(new Error(`ziral ${args[0]} exited with ${code}`)));
    });
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
    const exited = new Promise(resolve => chrome.once('exit', resolve)).then(code => {
        for (const {reject} of pending.values()) reject(new Error(`chromium exited with ${code}`));
        pending.clear();
    });
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
        if (chrome.exitCode !== null || chrome.signalCode !== null) return reject(new Error('chromium exited'));
        pending.set(++id, {resolve, reject});
        chrome.stdio[3].write(JSON.stringify({id, method, params, sessionId}) + '\0');
    });
    const close = async () => {
        chrome.kill();
        await exited;
    };
    return {send, close};
}

async function record(url, path) {
    const {send, close} = browse(new URL(url).hostname);
    try {
        const {targetId} = await send('Target.createTarget', {url: 'about:blank'});
        const {sessionId} = await send('Target.attachToTarget', {targetId, flatten: true});
        const evaluate = async expression => (await send('Runtime.evaluate', {expression, returnByValue: true, awaitPromise: true}, sessionId)).result.value;
        await send('Emulation.setDeviceMetricsOverride', {...SIZE, deviceScaleFactor: 1, mobile: false}, sessionId);
        const start = performance.now();
        await send('Page.navigate', {url: `${url}#bench`}, sessionId);
        const chunks = `(async () => {
            const url = performance.getEntriesByType('resource').map(entry => entry.name).find(name => name.endsWith('/web/record.js'));
            if (!url) return [];
            const {saved} = await import(url);
            return [...(await saved()).values()].map(text => JSON.parse(text));
        })()`;
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
        let now = 0;
        for (const mark of MARKS) {
            await sleep(mark - now);
            now = mark;
            await press('m', 'KeyM', 77);
        }
        await sleep(END - now);
        if (kept) {
            const {data} = await send('Page.captureScreenshot', {format: 'png'}, sessionId);
            writeFileSync(join(scratch, 'web.png'), Buffer.from(data, 'base64'));
        }
        await evaluate(`dispatchEvent(new Event('pagehide'))`);
        const saved = await evaluate(chunks);
        const end = performance.now();
        const sessions = new Set(saved.map(chunk => chunk.session));
        if (sessions.size !== 1) throw new Error(`expected one recorded session, found ${sessions.size}`);
        saved.sort((a, b) => a.start - b.start);
        writeFileSync(path, JSON.stringify({build: saved[0].build, seed: saved[0].seed, inputs: saved.flatMap(chunk => chunk.inputs)}));
        return {start, end};
    } finally {
        await close();
        rmSync(join(scratch, 'profile'), {recursive: true, force: true});
    }
}

async function windows(target, run, path, {start, end}) {
    const {duration_seconds: duration, marks} = JSON.parse(await ziral('--analyze', path));
    if (marks.length !== MARKS.length) throw new Error(`${target}: expected ${MARKS.length} marks, found ${marks.length}`);
    return marks.map(({window: [from, to], frames}) => {
        if (Math.abs(to - from - WINDOW) > 0.5) throw new Error(`${target}: window ${from}-${to} is not ${WINDOW} s`);
        const busiest = others(start + from * 1000, end - (duration - to) * 1000);
        console.log(`${target} run ${run}, ${from.toFixed(0)}-${to.toFixed(0)} s: ${frames.count} frames, p50 ${frames.p50_ms} ms, p90 ${frames.p90_ms} ms, p99 ${frames.p99_ms} ms, max ${frames.max_ms} ms, ${frames.over_budget} over budget, other work on up to ${busiest.toFixed(1)} of ${cpus.size} CPUs`);
        return {over: frames.over_budget, busy: busiest > BUSY};
    });
}

async function holds(target, measure) {
    const deadline = performance.now() + PATIENCE * 1000;
    for (let run = 1; ; run++) {
        const path = join(scratch, `${target}-${run}.json`);
        const measured = await windows(target, run, path, await measure(path));
        if (measured.every(({over}) => over === 0)) return true;
        if (measured.some(({over, busy}) => over && !busy)) return false;
        if (performance.now() > deadline) {
            console.log(`${target}: still over budget with other work on more than ${BUSY} CPUs after ${PATIENCE / 60} minutes`);
            return false;
        }
        console.log(`${target} run ${run} went over budget only while other work held more than ${BUSY} CPUs; measuring again once it holds fewer`);
        await settle(deadline);
    }
}

try {
    const native = await holds('native', async path => {
        const start = performance.now();
        await ziral('--bench', path, String(END), ...MARKS.map(String));
        return {start, end: performance.now()};
    });
    let url = page;
    let server;
    if (!/^https?:/.test(page)) {
        server = await serve(page);
        url = `http://127.0.0.1:${server.address().port}/`;
    }
    let web;
    try {
        web = await holds('web', path => record(url, path));
    } finally {
        server?.close();
    }
    process.exitCode = native && web ? 0 : 1;
} finally {
    if (!kept) rmSync(scratch, {recursive: true, force: true});
}
