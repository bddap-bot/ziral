import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import test from 'node:test';

const source = readFileSync(new URL('./record.js', import.meta.url), 'utf8').replaceAll('export function', 'function');

test('sessions survive reload and pagehide preserves the final inputs without IndexedDB', async () => {
    const stored = new Map();
    for (const id of ['first', 'second']) {
        const events = new Map();
        let now = 6000;
        const context = vm.createContext({
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
