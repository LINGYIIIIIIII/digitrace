import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sharp = createRequire(path.join(root, 'frontend', 'package.json'))('sharp');
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">
  <rect x="8" y="8" width="240" height="240" rx="58" fill="#463839"/>
  <path d="M30 128h45l17-1 20-58 21 126 20-67h28l16-1h29" fill="none" stroke="#ff4c2f" stroke-width="16" stroke-linecap="round" stroke-linejoin="round"/>
</svg>`;
const out = path.join(root, 'src-tauri', 'icons', 'icon-256.png');
await sharp(Buffer.from(svg)).png().toFile(out);
console.log('OK ' + out + ' bytes=' + fs.statSync(out).size);
