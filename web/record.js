let record;
let id = crypto.randomUUID();
let attempted = 0;
const database = new Promise((resolve, reject) => {
    const opening = indexedDB.open('ziral-records', 1);
    opening.onupgradeneeded = () => opening.result.createObjectStore('records');
    opening.onsuccess = () => resolve(opening.result);
    opening.onerror = () => reject(opening.error);
});
database.catch(() => {});

function persist() {
    if (!record) return;
    attempted = performance.now();
    const text = JSON.stringify(record);
    const savedId = id;
    queue_upload(text);
    database.then(db => {
        const transaction = db.transaction('records', 'readwrite');
        transaction.objectStore('records').put(text, savedId);
        transaction.onerror = () => console.error(transaction.error);
    }).catch(console.error);
    try { localStorage.setItem(`ziral-record-${savedId}`, text); } catch (error) { console.error(error); }
}

export function begin_record() {
    persist();
    record = undefined;
    id = crypto.randomUUID();
    attempted = 0;
}

export function finish_record() {
    persist();
}

export function append_record(build, seed, inputs) {
    record ??= {build, seed: Number(seed), token, endpoint, session: id, inputs: []};
    record.inputs.push(...JSON.parse(inputs));
    if (performance.now() - attempted >= 5000) persist();
}

addEventListener('pagehide', persist);
document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') persist();
});

const parameters = new URLSearchParams(globalThis.location?.hash.slice(1) ?? '');
const token = parameters.get('token');
const endpoint = parameters.get('endpoint');
const queued = new Map();
const delivered = new Map();
let transport;
let uploading = false;

export async function send_record(snapshot, connection, start = 0) {
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    const limit = snapshot.inputs.length;
    while (start < limit) {
        let end = Math.min(start + 512, limit);
        let bytes;
        do {
            bytes = encoder.encode(JSON.stringify({
                token: snapshot.token, session: snapshot.session,
                build: snapshot.build, seed: snapshot.seed,
                start, inputs: snapshot.inputs.slice(start, end),
            }));
            if (bytes.length <= 900000) break;
            if (end === start + 1) throw new Error('Input exceeds upload limit');
            end = start + Math.ceil((end - start) / 2);
        } while (true);
        await connection.send_only(bytes);
        const {next} = JSON.parse(decoder.decode(await connection.recv()));
        if (!Number.isSafeInteger(next) || next < end) throw new Error('Invalid upload acknowledgment');
        start = next;
    }
    return start;
}

function queue_upload(text) {
    let snapshot;
    try { snapshot = JSON.parse(text); } catch { return; }
    if (!snapshot || !/^[a-f0-9]{192}$/.test(snapshot.token ?? '')
        || typeof snapshot.endpoint !== 'string' || !snapshot.endpoint
        || !/^[a-f0-9-]{36}$/.test(snapshot.session ?? '')
        || !/^[a-f0-9]{40}$/.test(snapshot.build ?? '')
        || snapshot.seed !== 0 || !Array.isArray(snapshot.inputs) || !snapshot.inputs.length) return;
    const previous = queued.get(snapshot.session);
    if (!previous || previous.inputs.length < snapshot.inputs.length) queued.set(snapshot.session, snapshot);
    upload();
}

async function upload() {
    if (uploading || !queued.size) return;
    uploading = true;
    try {
        transport ??= import('https://bddap-bot.github.io/botq/botq_dash_wasm.js').then(async connection => {
            await connection.default();
            await connection.init();
            return connection;
        });
        const connection = await transport;
        for (const [session, snapshot] of [...queued]) {
            try {
                await connection.connect(snapshot.endpoint, 'ziral-record/1');
                const next = await send_record(snapshot, connection, delivered.get(session) ?? 0);
                delivered.set(session, next);
                if (queued.get(session) === snapshot) queued.delete(session);
            } catch (error) { console.error(error); }
        }
    } catch (error) {
        transport = undefined;
        console.error(error);
    } finally {
        uploading = false;
    }
}

database.then(db => {
    const request = db.transaction('records', 'readonly').objectStore('records').getAll();
    request.onsuccess = () => request.result.forEach(queue_upload);
    request.onerror = () => console.error(request.error);
}).catch(console.error);
try {
    for (let index = 0; index < localStorage.length; index++) {
        const key = localStorage.key(index);
        if (key?.startsWith('ziral-record-')) queue_upload(localStorage.getItem(key));
    }
} catch (error) { console.error(error); }

setInterval(upload, 5000);
