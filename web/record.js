let live;
let attempted = 0;
let bench = false;
const database = new Promise((resolve, reject) => {
    const opening = indexedDB.open('ziral-records', 1);
    opening.onupgradeneeded = () => opening.result.createObjectStore('records');
    opening.onsuccess = () => resolve(opening.result);
    opening.onerror = () => reject(opening.error);
});
database.catch(() => {});

const pending = new Map();
const delivered = new Map();
let uploading = false;

function name(chunk) {
    return `ziral-record-${chunk.session}-${chunk.start}`;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();
let unsealed = new Uint8Array(1 << 15);
let length = 0;

function seal() {
    if (!live?.count) return;
    attempted = performance.now();
    const {count, ...chunk} = live;
    live = {...chunk, start: chunk.start + count, count: 0};
    const header = JSON.stringify(bench ? {...chunk, bench} : chunk);
    const text = `${header.slice(0, -1)},"inputs":[${decoder.decode(unsealed.subarray(0, length))}]}`;
    length = 0;
    const key = name(chunk);
    try { localStorage.setItem(key, text); } catch (error) { console.error(error); }
    database.then(db => store(db, key, text)).catch(console.error);
    if (bench) return;
    queue(key, text);
    upload();
}

function store(db, key, text) {
    const transaction = db.transaction('records', 'readwrite');
    transaction.objectStore('records').put(text, key);
    committed(transaction, key);
}

function committed(transaction, key) {
    transaction.oncomplete = () => {
        try { localStorage.removeItem(key); } catch (error) { console.error(error); }
    };
    transaction.onerror = () => console.error(transaction.error);
}

export function begin_record(build, seed) {
    seal();
    live = {build, seed: Number(seed), session: crypto.randomUUID(), start: 0, count: 0};
    attempted = 0;
}

export function finish_record() {
    seal();
}

export function bench_record() {
    seal();
    bench = true;
}

export function append_record(inputs, count) {
    if (!count) return;
    if (!write(inputs)) {
        seal();
        if (!write(inputs)) {
            const kept = unsealed;
            unsealed = new Uint8Array(inputs.length);
            write(inputs);
            live.count = count;
            seal();
            unsealed = kept;
            return;
        }
    }
    live.count += count;
    if (performance.now() - attempted >= 5000) seal();
}

function write(inputs) {
    const at = length && length + 1;
    if (at + inputs.length > unsealed.length) return false;
    if (length) unsealed[length] = 44;
    unsealed.set(inputs, at);
    length = at + inputs.length;
    return true;
}

addEventListener('pagehide', seal);
document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') seal();
});

const endpoint = globalThis.ZIRAL_RECORDS_ENDPOINT;
let transport;
async function request_record(bytes) {
    transport ??= import(new URL('./ziral_records_transport.js', globalThis.location.href)).then(async module => {
        await module.default();
        return module;
    }).catch(error => { transport = undefined; throw error; });
    const module = await transport;
    return JSON.parse(new TextDecoder().decode(await module.upload(endpoint, bytes)));
}

export async function send_record(chunk, request = request_record, start = chunk.start) {
    const limit = chunk.start + chunk.inputs.length;
    while (start < limit) {
        let end = Math.min(start + 512, limit);
        let bytes;
        do {
            bytes = encoder.encode(JSON.stringify({
                session: chunk.session,
                build: chunk.build, seed: chunk.seed,
                start, inputs: chunk.inputs.slice(start - chunk.start, end - chunk.start),
            }));
            if (bytes.length <= 900000) break;
            if (end === start + 1) throw new Error('Input exceeds upload limit');
            end = start + Math.ceil((end - start) / 2);
        } while (true);
        const response = await request(bytes);
        if (response.error) throw new Error(response.error);
        const {next} = response;
        if (!Number.isSafeInteger(next) || next < end) throw new Error('Invalid upload acknowledgment');
        start = next;
    }
    return start;
}

function queue(key, text) {
    let chunk;
    try { chunk = {start: 0, ...JSON.parse(text)}; } catch { return; }
    if (chunk.bench) return forget(key);
    if (!/^[a-f0-9-]{36}$/.test(chunk.session ?? '')
        || !/^[a-f0-9]{40}$/.test(chunk.build ?? '')
        || chunk.seed !== 0 || !Number.isSafeInteger(chunk.start) || chunk.start < 0
        || !Array.isArray(chunk.inputs) || !chunk.inputs.length) return;
    pending.set(key, {session: chunk.session, start: chunk.start, end: chunk.start + chunk.inputs.length, text});
}

function forget(key) {
    pending.delete(key);
    try { localStorage.removeItem(key); } catch (error) { console.error(error); }
    database.then(db => {
        const transaction = db.transaction('records', 'readwrite');
        transaction.objectStore('records').delete(key);
        transaction.onerror = () => console.error(transaction.error);
    }).catch(console.error);
}

async function upload() {
    if (uploading || !pending.size) return;
    uploading = true;
    try {
        const blocked = new Set();
        const ordered = [...pending].sort(([, a], [, b]) => a.session.localeCompare(b.session) || a.start - b.start);
        for (const [key, entry] of ordered) {
            if (blocked.has(entry.session)) continue;
            try {
                const from = Math.max(entry.start, delivered.get(entry.session) ?? 0);
                if (from < entry.end) {
                    delivered.set(entry.session, await send_record({start: 0, ...JSON.parse(entry.text)}, request_record, from));
                }
                if (pending.get(key) === entry) forget(key);
            } catch (error) {
                blocked.add(entry.session);
                console.error(error);
            }
        }
    } finally {
        uploading = false;
    }
}

function kept() {
    const stored = new Map();
    try {
        for (let index = 0; index < localStorage.length; index++) {
            const key = localStorage.key(index);
            if (key?.startsWith('ziral-record-')) stored.set(key, localStorage.getItem(key));
        }
    } catch (error) { console.error(error); }
    return stored;
}

export async function saved() {
    const stored = kept();
    try {
        const db = await database;
        await new Promise((resolve, reject) => {
            const request = db.transaction('records', 'readonly').objectStore('records').openCursor();
            request.onsuccess = () => {
                const cursor = request.result;
                if (!cursor) return resolve();
                stored.set(String(cursor.key), cursor.value);
                cursor.continue();
            };
            request.onerror = () => reject(request.error);
        });
    } catch (error) { console.error(error); }
    return stored;
}

for (const [key, text] of kept()) queue(key, text);
saved().then(stored => {
    for (const [key, text] of stored) queue(key, text);
    upload();
});

setInterval(upload, 5000);
