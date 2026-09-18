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
            URLSearchParams, TextEncoder, TextDecoder, setInterval() {},
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
        URLSearchParams, TextEncoder, TextDecoder, setInterval() {},
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
        URLSearchParams, TextEncoder, TextDecoder, setInterval() {},
        crypto: {randomUUID: () => 'a'.repeat(36)},
        performance: {now: () => 0},
        localStorage: {setItem() {}},
        addEventListener() {},
        document: {addEventListener() {}},
        console: {error() {}},
    });
    vm.runInContext(source, context);
    const snapshot = {
        token: 'c'.repeat(192), session: 'a'.repeat(36), build: 'b'.repeat(40), seed: 0,
        inputs: Array.from({length: 1200}, (_, index) => [index, {f: 1}]),
    };
    const batches = [];
    const connection = {
        async send_only(bytes) { batches.push(JSON.parse(new TextDecoder().decode(bytes))); },
        async recv() {
            const batch = batches.at(-1);
            return new TextEncoder().encode(JSON.stringify({next: batch.start + batch.inputs.length}));
        },
    };
    assert.equal(await context.send_record(snapshot, connection, 100), 1200);
    assert.deepEqual(batches.map(batch => batch.start), [100, 612, 1124]);
    assert.deepEqual(batches.flatMap(batch => batch.inputs), snapshot.inputs.slice(100));
    for (const batch of batches) {
        assert.equal(batch.token, snapshot.token);
        assert.equal(batch.session, snapshot.session);
        assert.equal(batch.build, snapshot.build);
    }
    connection.recv = async () => new TextEncoder().encode('{"next":0}');
    await assert.rejects(context.send_record(snapshot, connection), /Invalid upload acknowledgment/);
});

test('a rejected saved session cannot block another token or an idle retry', async () => {
    let retry;
    let current;
    const delivered = [];
    const connection = {
        async connect(endpoint) { current = endpoint; if (endpoint === 'bad') throw new Error('rejected'); },
        async send_only() { delivered.push(current); },
        async recv() { return new TextEncoder().encode('{"next":1}'); },
    };
    const context = vm.createContext({
        URLSearchParams, TextEncoder, TextDecoder,
        setInterval(callback) { retry = callback; },
        crypto: {randomUUID: () => 'a'.repeat(36)},
        performance: {now: () => 0},
        localStorage: {setItem() {}},
        addEventListener() {},
        document: {addEventListener() {}},
        console: {error() {}}, connection,
    });
    vm.runInContext(source, context);
    vm.runInContext('transport = Promise.resolve(connection)', context);
    const snapshot = {token: 'c'.repeat(192), session: 'a'.repeat(36), build: 'b'.repeat(40), seed: 0, inputs: [[0, 'Refill']]};
    for (const text of ['null', '{}', JSON.stringify({...snapshot, endpoint: 'bad'}), JSON.stringify({...snapshot, session: 'd'.repeat(36), endpoint: 'good'})]) {
        context.queue_upload(text);
    }
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(delivered, ['good']);
    connection.connect = async endpoint => { current = endpoint; };
    await retry();
    assert.deepEqual(delivered, ['good', 'bad']);
});

test('ack deletes both saved copies but preserves a newer snapshot and failed uploads', async () => {
    const saved = new Map();
    const disk = new Map();
    const opening = {};
    let acknowledge;
    const connection = {
        async connect() {},
        async send_only() {},
        recv: () => new Promise(resolve => { acknowledge = resolve; }),
    };
    const context = vm.createContext({
        URLSearchParams, TextEncoder, TextDecoder, setInterval() {},
        crypto: {randomUUID: () => 'a'.repeat(36)},
        performance: {now: () => 6000},
        localStorage: {
            getItem: key => saved.get(key), setItem: (key, value) => saved.set(key, value),
            removeItem: key => saved.delete(key),
        },
        indexedDB: {open: () => opening},
        addEventListener() {}, document: {addEventListener() {}},
        console: {error() {}}, connection,
    });
    vm.runInContext(source, context);
    vm.runInContext('transport = Promise.resolve(connection)', context);
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
    const snapshot = {token: 'c'.repeat(192), endpoint: 'test', session: 'a'.repeat(36), build: 'b'.repeat(40), seed: 0, inputs: [[0, 'Refill']]};
    const key = `ziral-record-${snapshot.session}`;
    const save = value => { const text = JSON.stringify(value); saved.set(key, text); disk.set(snapshot.session, text); return text; };
    context.queue_upload(save(snapshot));
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(saved.size, 1);
    assert.equal(disk.size, 1);
    const newer = {...snapshot, inputs: [...snapshot.inputs, [0, 'Refill']]};
    save(newer);
    acknowledge(new TextEncoder().encode('{"next":1}'));
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(JSON.parse(saved.get(key)).inputs.length, 2);
    assert.equal(JSON.parse(disk.get(snapshot.session)).inputs.length, 2);
    connection.recv = async () => new TextEncoder().encode('{"next":0}');
    context.queue_upload(JSON.stringify(newer));
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(saved.size, 1);
    assert.equal(disk.size, 1);
    connection.recv = async () => new TextEncoder().encode('{"next":2}');
    await context.upload();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(saved.size, 0);
    assert.equal(disk.size, 0);
    vm.runInContext(`record = ${JSON.stringify(newer)}; persist()`, context);
    assert.equal(saved.size, 0);
});
