import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import test from 'node:test';

const source = readFileSync(new URL('./record.js', import.meta.url), 'utf8').replaceAll('export function', 'function').replaceAll('export async function', 'async function');

const session = 'a'.repeat(36);
const build = 'b'.repeat(40);

function page(overrides) {
    const context = vm.createContext({
        URLSearchParams, TextEncoder, TextDecoder, AbortSignal, setInterval() {},
        crypto: {randomUUID: () => session},
        performance: {now: () => 6000},
        localStorage: {setItem() {}, getItem() { return null; }, removeItem() {}},
        addEventListener() {}, document: {addEventListener() {}}, console: {error() {}},
        ...overrides,
    });
    vm.runInContext(source, context);
    return context;
}

function storage() {
    const saved = new Map();
    return {saved, localStorage: {
        get length() { return saved.size; },
        key: index => [...saved.keys()][index],
        getItem: key => saved.get(key) ?? null,
        setItem: (key, value) => saved.set(key, value),
        removeItem: key => saved.delete(key),
    }};
}

const settle = () => new Promise(resolve => setImmediate(resolve));

test('sessions survive reload and pagehide preserves the final inputs without IndexedDB', async () => {
    const stored = new Map();
    for (const id of ['first', 'second']) {
        const events = new Map();
        let now = 6000;
        const context = page({
            crypto: {randomUUID: () => id},
            performance: {now: () => now},
            localStorage: {setItem: (key, value) => stored.set(key, value)},
            addEventListener: (name, callback) => events.set(name, callback),
        });
        vm.runInContext(`append_record('build', 0n, '[[0.1,{"f":0.1}]]')`, context);
        now += 1;
        vm.runInContext(`append_record('build', 0n, '[[0.2,{"f":0.1}]]')`, context);
        events.get('pagehide')?.();
        await settle();
        const first = JSON.parse(stored.get(`ziral-record-${id}-0`));
        const last = JSON.parse(stored.get(`ziral-record-${id}-1`));
        assert.deepEqual([first.start, first.inputs.length], [0, 1]);
        assert.deepEqual([last.start, last.inputs.length], [1, 1]);
        assert.equal(last.inputs[0][0], 0.2);
    }
    assert.equal(stored.size, 4);
});

test('each seal stores only the inputs appended since the previous seal', async () => {
    const {saved, localStorage} = storage();
    let now = 6000;
    const context = page({performance: {now: () => now}, localStorage});
    context.request_record = () => new Promise(() => {});
    context.append_record(build, 0, JSON.stringify([[0, {f: 0}]]));
    for (let frame = 1; frame <= 2000; frame++) {
        now += 5;
        context.append_record(build, 0, JSON.stringify([[frame, {f: 0.005}]]));
    }
    await settle();
    const chunks = [...saved.values()].map(text => JSON.parse(text));
    assert.equal(chunks.length, 3);
    assert.deepEqual(chunks.map(chunk => chunk.start), [0, 1, 1001]);
    assert.deepEqual(chunks.map(chunk => chunk.inputs.length), [1, 1000, 1000]);
    assert.deepEqual(chunks.flatMap(chunk => chunk.inputs.map(([at]) => at)), Array.from({length: 2001}, (_, at) => at));
    assert.equal(vm.runInContext('live.inputs.length', context), 0);
});

test('a sealed chunk stays in localStorage only until IndexedDB commits it', async () => {
    const {saved, localStorage} = storage();
    const disk = new Map();
    const opening = {};
    const transactions = [];
    const context = page({localStorage, indexedDB: {open: () => opening}});
    opening.result = {transaction: () => {
        const transaction = {objectStore: () => ({
            openCursor: () => ({}),
            put: (text, key) => disk.set(key, text),
        })};
        transactions.push(transaction);
        return transaction;
    }};
    opening.onsuccess();
    context.request_record = () => new Promise(() => {});
    context.append_record(build, 0, '[[0,"Refill"]]');
    await settle();
    assert.deepEqual([...saved.keys()], [`ziral-record-${session}-0`]);
    assert.deepEqual([...disk.keys()], [`ziral-record-${session}-0`]);
    for (const transaction of transactions) transaction.oncomplete?.();
    assert.equal(saved.size, 0);
    assert.deepEqual([...disk.keys()], [`ziral-record-${session}-0`]);
});

test('recording reset keeps deferred database writes under their original session', async () => {
    const stored = new Map();
    const opening = {};
    let next = 0;
    const context = page({
        crypto: {randomUUID: () => `session-${next++}`},
        indexedDB: {open: () => opening},
    });
    vm.runInContext(`begin_record(); append_record('build', 0n, '[[0.1,{"f":0.1}]]'); begin_record(); append_record('build', 0n, '[[0.2,{"f":0.2}]]');`, context);
    opening.result = {
        transaction: () => ({objectStore: () => ({put: (text, id) => stored.set(id, text)})}),
    };
    opening.onsuccess();
    await settle();
    assert.equal(stored.size, 2);
    assert.equal(JSON.parse(stored.get('ziral-record-session-1-0')).inputs[0][0], 0.1);
    assert.equal(JSON.parse(stored.get('ziral-record-session-2-0')).inputs[0][0], 0.2);
});

test('upload sends attributed prefixes and resumes at the durable acknowledgment', async () => {
    const context = page({performance: {now: () => 0}});
    const snapshot = {
        session, build, seed: 0, start: 0,
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
    batches.length = 0;
    const later = {...snapshot, start: 5000, inputs: snapshot.inputs.slice(0, 600)};
    assert.equal(await context.send_record(later, connection), 5600);
    assert.deepEqual(batches.map(batch => batch.start), [5000, 5512]);
    assert.deepEqual(batches.flatMap(batch => batch.inputs), later.inputs);
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
        if (endpoint === session && failing) throw new Error('rejected');
        delivered.push(endpoint);
        return {next: 1};
    };
    const context = page({setInterval(callback) { retry = callback; }, performance: {now: () => 0}});
    context.request_record = (...args) => connection(...args);
    const snapshot = {session, build, seed: 0, inputs: [[0, 'Refill']]};
    for (const [key, text] of [['null', 'null'], ['empty', '{}'], ['number', '5'], ['a', JSON.stringify(snapshot)], ['d', JSON.stringify({...snapshot, session: 'd'.repeat(36)})]]) {
        context.restore(key, text);
    }
    await settle();
    await retry();
    assert.deepEqual(delivered, ['d'.repeat(36)]);
    failing = false;
    await retry();
    assert.deepEqual(delivered, ['d'.repeat(36), session]);
});

test('an acknowledgment forgets only the chunks it covers, and a failed upload keeps them', async () => {
    const {saved, localStorage} = storage();
    const disk = new Map();
    const opening = {};
    let acknowledge;
    let connection = () => new Promise(resolve => { acknowledge = resolve; });
    const clock = {now: 6000};
    const context = page({localStorage, indexedDB: {open: () => opening}, performance: {now: () => clock.now}});
    opening.result = {transaction: () => ({objectStore: () => ({
        openCursor: () => ({}),
        put: (text, key) => disk.set(key, text),
        delete: key => disk.delete(key),
    })})};
    opening.onsuccess();
    context.request_record = (...args) => connection(...args);
    context.append_record(build, 0, '[[0,"Refill"]]');
    await settle();
    assert.deepEqual([...saved.keys()], [`ziral-record-${session}-0`]);
    assert.deepEqual([...disk.keys()], [`ziral-record-${session}-0`]);
    clock.now = 12000;
    context.append_record(build, 0, '[[0,"Refill"]]');
    await settle();
    assert.equal(saved.size, 2);
    connection = async () => ({error: 'record upload rejected'});
    acknowledge({next: 1});
    await settle();
    assert.deepEqual([...saved.keys()], [`ziral-record-${session}-1`]);
    assert.deepEqual([...disk.keys()], [`ziral-record-${session}-1`]);
    await context.upload();
    await settle();
    assert.equal(saved.size, 1);
    assert.equal(vm.runInContext('pending.size', context), 1);
    connection = async bytes => ({next: JSON.parse(new TextDecoder().decode(bytes)).start + 1});
    await context.upload();
    await settle();
    assert.equal(saved.size, 0);
    assert.equal(disk.size, 0);
    assert.equal(vm.runInContext('pending.size', context), 0);
});

test('a whole record saved before chunking uploads from its first input and is forgotten', async () => {
    const {saved, localStorage} = storage();
    const legacy = {session, build, seed: 0, inputs: [[0, 'Refill'], [0, 'Refill']]};
    saved.set(`ziral-record-${session}`, JSON.stringify(legacy));
    const sent = [];
    const context = page({localStorage});
    await settle();
    context.request_record = async bytes => {
        const batch = JSON.parse(new TextDecoder().decode(bytes));
        sent.push(batch);
        return {next: batch.start + batch.inputs.length};
    };
    await context.upload();
    await settle();
    assert.deepEqual(sent.map(batch => [batch.start, batch.inputs.length]), [[0, 2]]);
    assert.equal(saved.size, 0);
});

test('restored chunks upload from the earliest whatever order storage lists them in', async () => {
    const {saved, localStorage} = storage();
    for (const start of [3121, 0]) {
        saved.set(`ziral-record-${session}-${start}`, JSON.stringify({session, build, seed: 0, start, inputs: Array.from({length: start ? 1 : 3121}, () => [0, 'Refill'])}));
    }
    const starts = [];
    const context = page({localStorage});
    context.request_record = async bytes => {
        const batch = JSON.parse(new TextDecoder().decode(bytes));
        starts.push(batch.start);
        return {next: batch.start + batch.inputs.length};
    };
    await settle();
    await settle();
    assert.equal(starts[0], 0);
    assert.equal(starts.at(-1), 3121);
    assert.equal(saved.size, 0);
});

test('localStorage chunks restore even when IndexedDB never opens', async () => {
    const {saved, localStorage} = storage();
    saved.set(`ziral-record-${session}-0`, JSON.stringify({session, build, seed: 0, start: 0, inputs: [[0, 'Refill']]}));
    let retry;
    const starts = [];
    const context = page({localStorage, indexedDB: {open: () => ({})}, setInterval(callback) { retry = callback; }});
    context.request_record = async bytes => {
        const batch = JSON.parse(new TextDecoder().decode(bytes));
        starts.push(batch.start);
        return {next: batch.start + batch.inputs.length};
    };
    await retry();
    assert.deepEqual(starts, [0]);
    assert.equal(saved.size, 0);
});

test('the public page records and uploads without a fragment or credential', async () => {
    const sent = [];
    const context = page({
        location: {href: 'https://example.test/', hash: ''},
        ZIRAL_RECORDS_ENDPOINT: 'b'.repeat(64),
    });
    context.request_record = async bytes => {
        const batch = JSON.parse(new TextDecoder().decode(bytes));
        sent.push(batch);
        return {next: batch.start + batch.inputs.length};
    };
    context.append_record(build, 0, '[[0,"Refill"]]');
    await settle();
    assert.equal(sent.length, 1);
    assert.equal(sent[0].session, session);
    assert.deepEqual(Object.keys(sent[0]).sort(), ['build', 'inputs', 'seed', 'session', 'start']);
});
