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
    record ??= {build, seed: Number(seed), inputs: []};
    record.inputs.push(...JSON.parse(inputs));
    if (performance.now() - attempted >= 5000) persist();
}

addEventListener('pagehide', persist);
document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') persist();
});
