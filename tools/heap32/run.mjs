// Runs the wasm measurement and checks it against the firmware's internal heap.
import { readFileSync } from 'node:fs';

const here = new URL('.', import.meta.url).pathname;
const wasm = readFileSync(`${here}target/wasm32-unknown-unknown/release/orbit_heap32.wasm`);
const game = process.argv[2] ?? 'angler';
const { instance } = await WebAssembly.instantiate(wasm, {});
const ptr = instance.exports[`measure_${game}`]();
const [built, end, peak] = new Uint32Array(instance.exports.memory.buffer, ptr, 3);

const main = readFileSync(`${here}../../firmware/src/lib.rs`, 'utf8');
const kb = (name) => Number(main.match(new RegExp(`const ${name}: usize = (\\d+) \\* 1024;`))[1]) * 1024;
const reclaimed = Number(main.match(/ram\(reclaimed\)\] size: (\d+)/)[1]);
const band = 466 * 2 * Number(main.match(/const BAND_ROWS: usize = (\d+);/)[1]);
const internal = reclaimed + kb('INTERNAL_HEAP_BYTES');

const k = (n) => `${(n / 1024).toFixed(0)} KB`;
console.log(`${game}: after build ${k(built)}, after play ${k(end)}, peak ${k(peak)} (+${k(band)} render band)`);
console.log(`firmware internal heap ${k(internal)}: headroom at peak ${k(internal - peak - band)}`);
if (peak + band > internal) {
  console.log('OVER BUDGET: Bevy would spill into PSRAM on the device');
  process.exit(1);
}
