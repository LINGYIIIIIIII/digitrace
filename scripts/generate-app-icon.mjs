import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sharp = createRequire(path.join(root, 'frontend', 'package.json'))('sharp');
const output = path.join(root, 'src-tauri', 'icons', 'icon.ico');
const sizes = [16, 24, 32, 48, 64, 128, 256];

// Keep the Windows icon identical to the sidebar mark: rounded dark tile plus
// the orange-red activity trace, with enough contrast at 16px tray size.
const svg = (size) => `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 256 256">
  <rect x="8" y="8" width="240" height="240" rx="58" fill="#463839"/>
  <path d="M30 128h45l17-1 20-58 21 126 20-67h28l16-1h29" fill="none" stroke="#ff4c2f" stroke-width="16" stroke-linecap="round" stroke-linejoin="round"/>
</svg>`;

const pngs = await Promise.all(
  sizes.map((size) => sharp(Buffer.from(svg(size))).png().toBuffer()),
);

const header = Buffer.alloc(6);
header.writeUInt16LE(0, 0);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(sizes.length, 4);

const directory = Buffer.alloc(16 * sizes.length);
let offset = header.length + directory.length;
for (let i = 0; i < sizes.length; i += 1) {
  const size = sizes[i];
  const entry = i * 16;
  directory.writeUInt8(size === 256 ? 0 : size, entry);
  directory.writeUInt8(size === 256 ? 0 : size, entry + 1);
  directory.writeUInt8(0, entry + 2);
  directory.writeUInt8(0, entry + 3);
  directory.writeUInt16LE(1, entry + 4);
  directory.writeUInt16LE(32, entry + 6);
  directory.writeUInt32LE(pngs[i].length, entry + 8);
  directory.writeUInt32LE(offset, entry + 12);
  offset += pngs[i].length;
}

fs.writeFileSync(output, Buffer.concat([header, directory, ...pngs]));
console.log(`Generated ${output} (${sizes.join(', ')}px)`);
