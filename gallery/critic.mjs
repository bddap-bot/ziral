import { readFileSync, writeFileSync, existsSync, mkdirSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';

const root = resolve(process.argv[2]);
rmSync(`${root}/approved-hashes.txt`, { force: true });
const parse = text => new Map(text.trim().split('\n').filter(Boolean).map(line => line.split(' ')));
const manifest = readFileSync(`${root}/hashes.txt`, 'utf8');
const current = parse(manifest);
const baseline = parse(existsSync('gallery/hashes.txt') ? readFileSync('gallery/hashes.txt', 'utf8') : '');
if (readFileSync(`${root}/passed`, "utf8") !== readFileSync(`${root}/hashes.txt`, "utf8")) throw Error("gallery gates have no matching green result");
const rubric = readFileSync('gallery/RUBRIC.md', 'utf8');
const schema = {
    type: 'object', properties: {
        score: { type: 'integer', minimum: 0, maximum: 10 },
        issues: { type: 'array', items: { type: 'string' } }
    }, required: ['score', 'issues'], additionalProperties: false
};
writeFileSync(`${root}/schema.json`, JSON.stringify(schema));
const validate = (name, hash) => {
    if (readFileSync(`${root}/hashes.txt`, 'utf8') !== manifest || readFileSync(`${root}/passed`, 'utf8') !== manifest) throw Error('gallery changed during review');
    if (createHash('sha256').update(readFileSync(`${root}/${name}.png`)).digest('hex') !== hash) throw Error(`stale scene hash: ${name}`);
    if (readFileSync('gallery/RUBRIC.md', 'utf8') !== rubric) throw Error('rubric changed during review');
};
const resultAt = (path, name) => {
    const result = JSON.parse(readFileSync(path, 'utf8'));
    if (!Number.isInteger(result.score) || result.score < 0 || result.score > 10 || !Array.isArray(result.issues) || result.issues.some(issue => typeof issue !== 'string')) throw Error(`invalid critic result for ${name}`);
    return result;
};
const cache = resolve('.gallery/reviews');
mkdirSync(cache, { recursive: true });
const scores = [];
let changed = 0;
let passed = true;
for (const [name, hash] of current) {
    if (!/^[a-z0-9-]+$/.test(name)) throw Error("invalid scene name");
    validate(name, hash);
    if (baseline.get(name) === hash) continue;
    changed++;
    const image = `${root}/${name}.png`;
    const key = createHash('sha256').update(name + '\n' + hash + '\n' + rubric).digest('hex');
    const output = `${cache}/${key}.json`;
    if (!existsSync(output)) {
        const pending = `${root}/score.json`;
        const prompt = `Evaluate this scene only. Do not edit files. Scene: ${name}.\n\n${rubric}`;
        const run = spawnSync('codex', ['exec', prompt, '--ephemeral', '--sandbox', 'read-only', '--output-schema', `${root}/schema.json`, '--output-last-message', pending, '--image', image], { stdio: ['ignore', 'inherit', 'inherit'] });
        if (run.status !== 0) throw Error(`critic failed for ${name}`);
        const result = resultAt(pending, name);
        validate(name, hash);
        writeFileSync(output, JSON.stringify(result));
    }
    const result = resultAt(output, name);
    const line = `${name}: ${result.score}/10 ${result.issues.join('; ')}`;
    scores.push(line);
    console.log(line);
    passed &&= result.score >= 8;
}
for (const [name, hash] of current) validate(name, hash);
for (const name of baseline.keys()) if (!current.has(name)) throw Error(`baseline scene missing: ${name}`);
if (!changed) { scores.push('gallery unchanged'); console.log('gallery unchanged'); }
writeFileSync(`${root}/scores.txt`, scores.join('\n') + '\n');
if (!passed) throw Error('a changed scene scored below eight; hashes remain unapproved');
writeFileSync(`${root}/approved-hashes.txt`, manifest);
console.log(`Approved hashes: ${root}/approved-hashes.txt; copy to gallery/hashes.txt with this change.`);
