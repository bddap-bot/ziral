import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import test from 'node:test';

const source = readFileSync(new URL('./record.js', import.meta.url), 'utf8').replaceAll('export function', 'function').replaceAll('export async function', 'async function');

test('sessions survive reload and pagehide preserves the final inputs without IndexedDB', async () => {
    const stored = new Map();
    for (const id of ['first', 'second']) {
        const events = new Map();
        let now = 6000;
        const context = vm.createContext({
            URLSearchParams, TextEncoder, TextDecoder, AbortSignal, setInterval() {},
        crypto: {randomUUID: () => id},
            performance: {now: () => now},
            localStorage: {setItem: (key, value) => stored.set(key, value)},
            addEventListener: (name, callback) => events.set(name, callback),
            document: {addEventListener() {}},
            console: {error() {}},
        });
        vm.runInContext(source, context);
        vm.runInContext(`append_record('build', 0n, '[[0.1,{"f":0.1}]]')`, context);
        now += 1;
        vm.runInContext(`append_record('build', 0n, '[[0.2,{"f":0.1}]]')`, context);
        events.get('pagehide')?.();
        await new Promise(resolve => setImmediate(resolve));
        const record = JSON.parse(stored.get(`ziral-record-${id}`));
        assert.equal(record.inputs.length, 2);
        assert.equal(record.inputs[1][0], 0.2);
    }
    assert.equal(stored.size, 2);
});

test('recording reset keeps deferred database writes under their original session', async () => {
    const stored = new Map();
    const opening = {};
    let next = 0;
    const context = vm.createContext({
        URLSearchParams, TextEncoder, TextDecoder, AbortSignal, setInterval() {},
        crypto: {randomUUID: () => `session-${next++}`},
        performance: {now: () => 6000},
        localStorage: {setItem() {}},
        indexedDB: {open: () => opening},
        addEventListener() {},
        document: {addEventListener() {}},
        console: {error() {}},
    });
    vm.runInContext(source, context);
    vm.runInContext(`begin_record(); append_record('build', 0n, '[[0.1,{"f":0.1}]]'); begin_record(); append_record('build', 0n, '[[0.2,{"f":0.2}]]');`, context);
    opening.result = {
        transaction: () => ({objectStore: () => ({put: (text, id) => stored.set(id, text)})}),
    };
    opening.onsuccess();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(stored.size, 2);
    assert.equal(JSON.parse(stored.get('session-1')).inputs[0][0], 0.1);
    assert.equal(JSON.parse(stored.get('session-2')).inputs[0][0], 0.2);
});

test('upload sends attributed prefixes and resumes at the durable acknowledgment', async () => {
    const context = vm.createContext({
        URLSearchParams, TextEncoder, TextDecoder, AbortSignal, setInterval() {},
        crypto: {randomUUID: () => 'a'.repeat(36)},
        performance: {now: () => 0},
        localStorage: {setItem() {}},
        addEventListener() {},
        document: {addEventListener() {}},
        console: {error() {}},
    });
    vm.runInContext(source, context);
    const snapshot = {
        session: 'a'.repeat(36), build: 'b'.repeat(40), seed: 0,
        inputs: Array.from({length: 1200}, (_, index) => [index, {f: 1}]),
    };
    const batches = [];
    const connection = async body => {
        const batch = JSON.parse(new TextDecoder().decode(body));
        batches.push(batch);
        return {next: batch.start + batch.inputs.length};
    };
    assert.equal(await context.send_record(snapshot, connection, 100), 1200);
    assert.deepEqual(batches.map(batch => batch.start), [100, 612, 1124]);
    assert.deepEqual(batches.flatMap(batch => batch.inputs), snapshot.inputs.slice(100));
    for (const batch of batches) {
        assert.equal(batch.session, snapshot.session);
        assert.equal(batch.build, snapshot.build);
    }
    const invalid = async () => ({next: 0});
    await assert.rejects(context.send_record(snapshot, invalid), /Invalid upload acknowledgment/);
    await assert.rejects(context.send_record(snapshot, async () => ({
        error: 'record upload rejected', next: 1200,
    })), /record upload rejected/);
});

test('a rejected saved session cannot block another session or an idle retry', async () => {
    let retry;
    let failing = true;
    const delivered = [];
    const connection = async bytes => {
        const endpoint = JSON.parse(new TextDecoder().decode(bytes)).session;
        if (endpoint === 'a'.repeat(36) && failing) throw new Error('rejected');
        delivered.push(endpoint);
        return {next: 1};
    };
    const context = vm.createContext({
        URLSearchParams, TextEncoder, TextDecoder, AbortSignal,
        setInterval(callback) { retry = callback; },
        crypto: {randomUUID: () => 'a'.repeat(36)},
        performance: {now: () => 0},
        localStorage: {setItem() {}},
        addEventListener() {},
        document: {addEventListener() {}},
        console: {error() {}},
    });
    vm.runInContext(source, context);
    context.request_record = (...args) => connection(...args);
    const snapshot = {session: 'a'.repeat(36), build: 'b'.repeat(40), seed: 0, inputs: [[0, 'Refill']]};
    for (const text of ['null', '{}', JSON.stringify(snapshot), JSON.stringify({...snapshot, session: 'd'.repeat(36)})]) {
        context.queue_upload(text);
    }
    await new Promise(resolve => setImmediate(resolve));
    await retry();
    assert.deepEqual(delivered, ['d'.repeat(36)]);
    failing = false;
    await retry();
    assert.deepEqual(delivered, ['d'.repeat(36), 'a'.repeat(36)]);
});

test('ack deletes both saved copies but preserves a newer snapshot and failed uploads', async () => {
    const saved = new Map();
    const disk = new Map();
    const opening = {};
    let acknowledge;
    let connection = () => new Promise(resolve => { acknowledge = resolve; });
    const context = vm.createContext({
        URLSearchParams, TextEncoder, TextDecoder, AbortSignal, setInterval() {},
        crypto: {randomUUID: () => 'a'.repeat(36)},
        performance: {now: () => 6000},
        localStorage: {
            getItem: key => saved.get(key), setItem: (key, value) => saved.set(key, value),
            removeItem: key => saved.delete(key),
        },
        indexedDB: {open: () => opening},
        addEventListener() {}, document: {addEventListener() {}},
        console: {error() {}},
    });
    vm.runInContext(source, context);
    opening.result = {transaction: () => ({objectStore: () => ({
        getAll: () => ({}),
        get(key) {
            const request = {};
            queueMicrotask(() => { request.result = disk.get(key); request.onsuccess(); });
            return request;
        },
        delete: key => disk.delete(key),
    })})};
    opening.onsuccess();
    const snapshot = {session: 'a'.repeat(36), build: 'b'.repeat(40), seed: 0, inputs: [[0, 'Refill']]};
    context.request_record = (...args) => connection(...args);
    const key = `ziral-record-${snapshot.session}`;
    const save = value => { const text = JSON.stringify(value); saved.set(key, text); disk.set(snapshot.session, text); return text; };
    context.queue_upload(save(snapshot));
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(saved.size, 1);
    assert.equal(disk.size, 1);
    const newer = {...snapshot, inputs: [...snapshot.inputs, [0, 'Refill']]};
    save(newer);
    acknowledge({next: 1});
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(JSON.parse(saved.get(key)).inputs.length, 2);
    assert.equal(JSON.parse(disk.get(snapshot.session)).inputs.length, 2);
    connection = async () => ({error: 'record upload rejected', next: 2});
    context.queue_upload(JSON.stringify(newer));
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(saved.size, 1);
    assert.equal(disk.size, 1);
    connection = async () => ({next: 2});
    await context.upload();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(saved.size, 0);
    assert.equal(disk.size, 0);
    vm.runInContext(`record = ${JSON.stringify(newer)}; persist()`, context);
    assert.equal(saved.size, 0);
});

test('the public page records and uploads without a fragment or credential', async () => {
    const sent = [];
    const context = vm.createContext({
        TextEncoder, TextDecoder, setInterval() {},
        crypto: {randomUUID: () => 'a'.repeat(36)},
        performance: {now: () => 6000},
        location: {href: 'https://example.test/', hash: ''},
        ZIRAL_RECORDS_ENDPOINT: 'b'.repeat(64),
        localStorage: {setItem() {}, getItem() { return null; }, removeItem() {}},
        addEventListener() {}, document: {addEventListener() {}}, console: {error() {}},
    });
    vm.runInContext(source, context);
    context.request_record = async bytes => {
        const batch = JSON.parse(new TextDecoder().decode(bytes));
        sent.push(batch);
        return {next: batch.start + batch.inputs.length};
    };
    context.append_record('b'.repeat(40), 0, '[[0,"Refill"]]');
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(sent.length, 1);
    assert.equal(sent[0].session, 'a'.repeat(36));
    assert.deepEqual(Object.keys(sent[0]).sort(), ['build', 'inputs', 'seed', 'session', 'start']);
});
