let live;
let id = crypto.randomUUID();
let attempted = 0;
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

function seal() {
    if (!live?.inputs.length) return;
    attempted = performance.now();
    const chunk = live;
    live = {...chunk, start: chunk.start + chunk.inputs.length, inputs: []};
    const text = JSON.stringify(chunk);
    const key = name(chunk);
    try { localStorage.setItem(key, text); } catch (error) { console.error(error); }
    database.then(db => {
        const transaction = db.transaction('records', 'readwrite');
        transaction.objectStore('records').put(text, key);
        transaction.oncomplete = () => {
            try { localStorage.removeItem(key); } catch (error) { console.error(error); }
        };
        transaction.onerror = () => console.error(transaction.error);
    }).catch(console.error);
    queue(key, chunk);
    upload();
}

export function begin_record() {
    seal();
    live = undefined;
    id = crypto.randomUUID();
    attempted = 0;
}

export function finish_record() {
    seal();
}

export function append_record(build, seed, inputs) {
    live ??= {build, seed: Number(seed), session: id, start: 0, inputs: []};
    live.inputs.push(...JSON.parse(inputs));
    if (performance.now() - attempted >= 5000) seal();
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
    const encoder = new TextEncoder();
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

function restore(key, text) {
    let chunk;
    try { chunk = JSON.parse(text); } catch { return; }
    queue(key, {start: 0, ...chunk});
}

function queue(key, chunk) {
    if (!chunk || !/^[a-f0-9-]{36}$/.test(chunk.session ?? '')
        || !/^[a-f0-9]{40}$/.test(chunk.build ?? '')
        || chunk.seed !== 0 || !Number.isSafeInteger(chunk.start) || chunk.start < 0
        || !Array.isArray(chunk.inputs) || !chunk.inputs.length) return;
    pending.set(key, chunk);
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
        for (const [key, chunk] of ordered) {
            if (blocked.has(chunk.session)) continue;
            try {
                const from = Math.max(chunk.start, delivered.get(chunk.session) ?? 0);
                if (from < chunk.start + chunk.inputs.length) {
                    delivered.set(chunk.session, await send_record(chunk, request_record, from));
                }
                if (pending.get(key) === chunk) forget(key);
            } catch (error) {
                blocked.add(chunk.session);
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

for (const [key, text] of kept()) restore(key, text);
saved().then(stored => {
    for (const [key, text] of stored) restore(key, text);
    upload();
});

setInterval(upload, 5000);
