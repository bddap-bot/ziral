import { createHash } from 'node:crypto';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { spawnSync } from 'node:child_process';

const hash = createHash('sha256').update('frame').digest('hex');
const script = resolve('gallery/critic.mjs');
function fixture(baseline, score) {
    mkdirSync('.gallery', { recursive: true });
    const root = mkdtempSync(resolve('.gallery/critic-test-'));
    mkdirSync(join(root, 'gallery'));
    mkdirSync(join(root, 'run'));
    mkdirSync(join(root, 'bin'));
    writeFileSync(join(root, 'gallery/hashes.txt'), baseline.replace('abc', hash));
    writeFileSync(join(root, 'gallery/RUBRIC.md'), 'Fixed rubric');
    writeFileSync(join(root, 'run/hashes.txt'), `palette ${hash}\n`);
    writeFileSync(join(root, 'run/passed'), `palette ${hash}\n`);
    writeFileSync(join(root, 'run/palette.png'), 'frame');
    writeFileSync(join(root, 'bin/codex'), `#!/usr/bin/env node
const fs = require('node:fs');
if (!process.argv.slice(3, process.argv.indexOf('--image')).some(arg => arg.startsWith('Evaluate this scene only.'))) throw Error('review prompt consumed by variadic image option');
fs.appendFileSync('calls', 'called\\n');
fs.writeFileSync(process.argv[process.argv.indexOf('--output-last-message') + 1], JSON.stringify({score:${score},issues:[]}));
`, { mode: 0o755 });
    const run = () => spawnSync(process.execPath, [script, 'run'], { cwd: root, env: { ...process.env, PATH: `${join(root, 'bin')}:${process.env.PATH}` }, encoding: 'utf8' });
    return { root, run };
}

test('unchanged frames spend no critic tokens', () => {
    const { root, run } = fixture('palette abc\n', 0);
    try {
        const result = run();
        assert.equal(result.status, 0, result.stderr);
        assert.match(result.stdout, /gallery unchanged/);
        assert.throws(() => readFileSync(join(root, 'calls')));
    } finally { rmSync(root, { recursive: true }); }
});

test('a score below eight blocks and reuses the same judgement without more tokens', () => {
    const { root, run } = fixture('palette old\n', 7);
    try {
        for (let i = 0; i < 2; i++) {
            const result = run();
            assert.notEqual(result.status, 0);
            assert.match(result.stderr, /below eight/);
        }
        assert.equal(readFileSync(join(root, 'calls'), 'utf8'), 'called\n');
        assert.equal(readFileSync(join(root, 'gallery/hashes.txt'), 'utf8'), 'palette old\n');
    } finally { rmSync(root, { recursive: true }); }
});

test('eight approves a changed scene once', () => {
    const { root, run } = fixture('palette old\n', 8);
    try {
        for (let i = 0; i < 2; i++) {
            const result = run();
            assert.equal(result.status, 0, result.stderr);
            assert.match(result.stdout, /palette: 8\/10/);
        }
        assert.equal(readFileSync(join(root, 'calls'), 'utf8'), 'called\n');
    } finally { rmSync(root, { recursive: true }); }
});

test('a frame replaced during judgement cannot receive cached approval', () => {
    const { root, run } = fixture('palette old\n', 8);
    try {
        const stub = join(root, 'bin/codex');
        writeFileSync(stub, readFileSync(stub, 'utf8') + "\nfs.writeFileSync('run/palette.png', 'different frame');\n");
        const result = run();
        assert.notEqual(result.status, 0);
        assert.match(result.stderr, /stale scene hash/);
        writeFileSync(join(root, 'run/palette.png'), 'frame');
        const second = run();
        assert.notEqual(second.status, 0);
        assert.equal(readFileSync(join(root, 'calls'), 'utf8'), 'called\ncalled\n');
    } finally { rmSync(root, { recursive: true }); }
});

test('later judgement cannot approve an earlier frame replaced during the run', () => {
    const { root, run } = fixture('palette old\n', 8);
    try {
        const second = createHash('sha256').update('second frame').digest('hex');
        const manifest = `palette ${hash}\nsecond ${second}\n`;
        writeFileSync(join(root, 'run/hashes.txt'), manifest);
        writeFileSync(join(root, 'run/passed'), manifest);
        writeFileSync(join(root, 'run/second.png'), 'second frame');
        const stub = join(root, 'bin/codex');
        writeFileSync(stub, readFileSync(stub, 'utf8') + "\nif (process.argv.some(arg => arg.includes('Scene: second.'))) fs.writeFileSync('run/palette.png', 'different frame');\n");
        const result = run();
        assert.notEqual(result.status, 0);
        assert.match(result.stderr, /stale scene hash: palette/);
        assert.throws(() => readFileSync(join(root, 'run/approved-hashes.txt')));
    } finally { rmSync(root, { recursive: true }); }
});


test('a failed rerun removes an earlier approval', () => {
    const { root, run } = fixture('palette old\n', 8);
    try {
        const first = run();
        assert.equal(first.status, 0, first.stderr);
        assert.equal(readFileSync(join(root, 'run/approved-hashes.txt'), 'utf8'), `palette ${hash}\n`);
        writeFileSync(join(root, 'run/passed'), 'different gate result');
        const second = run();
        assert.notEqual(second.status, 0);
        assert.match(second.stderr, /no matching green result/);
        assert.throws(() => readFileSync(join(root, 'run/approved-hashes.txt')));
    } finally { rmSync(root, { recursive: true }); }
});

test('a scene with an approved design is judged against that design and its brief', () => {
    const { root, run } = fixture('palette old\n', 8);
    try {
        mkdirSync(join(root, 'art/machines/palette'), { recursive: true });
        writeFileSync(join(root, 'art/machines/palette/design.png'), 'design');
        writeFileSync(join(root, 'art/machines/palette/design.md'), 'A frame of shelf fungus.');
        const stub = join(root, 'bin/codex');
        writeFileSync(stub, readFileSync(stub, 'utf8') + `
const images = process.argv.flatMap((arg, i) => process.argv[i - 1] === '--image' ? [arg] : []);
if (images[1] !== 'art/machines/palette/design.png') throw Error('design image missing: ' + images);
if (!process.argv.some(arg => arg.includes('approved design') && arg.includes('A frame of shelf fungus.'))) throw Error('brief missing');
`);
        const first = run();
        assert.equal(first.status, 0, first.stderr);
        writeFileSync(join(root, 'art/machines/palette/design.md'), 'A frame of brass.');
        writeFileSync(stub, readFileSync(stub, 'utf8').replace('A frame of shelf fungus.', 'A frame of brass.').replace('score:8', 'score:6'));
        const second = run();
        assert.notEqual(second.status, 0);
        assert.match(second.stderr, /below eight/);
        assert.equal(readFileSync(join(root, 'calls'), 'utf8'), 'called\ncalled\n');
    } finally { rmSync(root, { recursive: true }); }
});
