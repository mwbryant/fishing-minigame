// Plays the itch.io build in headless Chromium: the town boots, the pad's
// stick turns the board, a tap on the house walks in and its mic listens,
// holding Z switches the board off and Space turns it back on. Fails on any page error or a step that doesn't
// happen, and prints the frame time, sound and mic numbers.
// Usage: node web/smoke.mjs [SHOTS_DIR]   (after `trunk build --release`; a
//   PNG of each step goes to SHOTS_DIR when given, and SMOKE_VIDEO=DIR records
//   the run). Needs playwright-core installed globally with its Chromium.

import { execSync } from 'node:child_process';
import { mkdirSync, readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { extname, join } from 'node:path';

const site = new URL('../dist/', import.meta.url).pathname;
const shots = process.argv[2];
const SIZE = 466;
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };

const globalModules = execSync('npm root -g').toString().trim();
const { chromium } = createRequire(join(globalModules, '/'))('playwright-core');

const server = createServer((req, res) => {
  const path = join(site, new URL(req.url, 'http://x').pathname.replace(/\/$/, '/index.html'));
  try {
    const body = readFileSync(path);
    res.writeHead(200, { 'content-type': TYPES[extname(path)] ?? 'application/octet-stream' });
    res.end(body);
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, resolve));

const browser = await chromium.launch({
  args: [
    '--autoplay-policy=no-user-gesture-required',
    '--use-fake-ui-for-media-stream',
    '--use-fake-device-for-media-stream',
  ],
});
const context = await browser.newContext({
  viewport: { width: 560, height: 600 },
  permissions: ['microphone'],
  ...(process.env.SMOKE_VIDEO ? { recordVideo: { dir: process.env.SMOKE_VIDEO, size: { width: 560, height: 600 } } } : {}),
});
const page = await context.newPage();
const errors = [];
page.on('pageerror', (err) => errors.push(String(err)));
page.on('console', (msg) => msg.type() === 'error' && errors.push(msg.text()));

let failed = false;
function check(ok, what) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
  failed ||= !ok;
}

async function shot(name) {
  if (!shots) return;
  mkdirSync(shots, { recursive: true });
  await page.screenshot({ path: join(shots, `${name}.png`) });
}

// The canvas's data- attributes (web/src/browser.rs `Stats`).
const state = () =>
  page.evaluate(() => {
    const data = document.getElementById('orbit').dataset;
    return {
      playing: data.playing,
      off: data.off === 'true',
      steer: Number(data.steer ?? 0),
      lean: Number(data.lean ?? 0),
      frames: Number(data.frames ?? 0),
      frameMs: Number(data.frameMs ?? 0),
      spokenSecs: Number(data.spokenSecs ?? 0),
      heardSecs: Number(data.heardSecs ?? 0),
    };
  });

async function waitFor(what, test, secs) {
  for (let i = 0; i < secs * 10; i++) {
    if (test(await state())) return true;
    await page.waitForTimeout(100);
  }
  return false;
}

/// The page point over pixel (x, y) of the 466 x 466 canvas `id`.
async function over(id, x, y) {
  const box = await page.locator(`#${id}`).boundingBox();
  return [box.x + (x * box.width) / SIZE, box.y + (y * box.height) / SIZE];
}

/// A tap at panel pixel (x, y), held a tenth of a second.
async function tap(x, y) {
  await page.mouse.move(...(await over('orbit', x, y)));
  await page.mouse.down();
  await page.waitForTimeout(100);
  await page.mouse.up();
}

/// Pushes the pad's stick by (dx, dy) of a full push for `ms`, then lets go.
async function push(dx, dy, ms) {
  await page.mouse.move(...(await over('pad', 233, 233)));
  await page.mouse.down();
  await page.mouse.move(...(await over('pad', 233 + dx * 150, 233 + dy * 150)), { steps: 5 });
  await page.waitForTimeout(ms);
  await page.mouse.up();
}

await page.goto(`http://localhost:${server.address().port}/`);
check(await waitFor('town', (s) => s.playing === 'town' && s.frames > 30, 10), 'the town runs');
await shot('town');

await push(1, 0, 600);
await page.waitForTimeout(500);
let turned = await state();
check(turned.steer > 20, `pushing the stick right turns the board and it stays turned (${turned.steer} deg)`);
await shot('pad-turned');
await push(-1, 0, 600);
await page.waitForTimeout(300);
turned = await state();
check(Math.abs(turned.steer) < 10, `pushing it left turns it back (${turned.steer} deg)`);

await tap(233, 110);
check(await waitFor('home', (s) => s.playing === 'home', 15), 'tapping the house walks in');
await page.waitForTimeout(2000);
await shot('house');
check(await waitFor('mic', (s) => s.heardSecs > 0, 5), 'the house hears the mic');

await page.keyboard.down('KeyZ');
await page.waitForTimeout(1500);
await shot('hold-to-sleep');
check(await waitFor('off', (s) => s.off, 8), 'holding Z switches the board off');
await page.keyboard.up('KeyZ');
await page.waitForTimeout(500);
await shot('dark');
await page.keyboard.press('Space');
check(await waitFor('on', (s) => !s.off && s.playing === 'town', 3), 'Space turns it back on in the town');
await page.waitForTimeout(1000);

const now = await state();
console.log(
  `frame ${now.frameMs.toFixed(3)} ms in wasm, ${now.frames} frames, ` +
    `${now.spokenSecs} s of sound, ${now.heardSecs} s of mic`,
);
check(errors.length === 0, `no page errors${errors.length ? `: ${errors.join(' | ')}` : ''}`);

await context.close();
await browser.close();
server.close();
console.log(failed ? 'SMOKE FAIL' : 'SMOKE PASS');
process.exit(failed ? 1 : 0);
