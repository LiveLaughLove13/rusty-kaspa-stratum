/**
 * Rasterize ui/dashboard/assets/kaspa.svg to a 1024×1024 PNG (letterboxed, transparent),
 * then run: npx tauri icon src-tauri/app-icon.png -o src-tauri/icons
 */
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const root = path.join(__dirname, '..');
const svgPath = path.join(root, 'ui', 'dashboard', 'assets', 'kaspa.svg');
const outPath = path.join(root, 'src-tauri', 'app-icon.png');

async function main() {
  let sharp;
  try {
    sharp = require('sharp');
  } catch {
    console.error('Install dev dependency: npm install sharp --save-dev');
    process.exit(1);
  }

  if (!fs.existsSync(svgPath)) {
    console.error('Missing SVG:', svgPath);
    process.exit(1);
  }

  await sharp(svgPath)
    .resize(1024, 1024, {
      fit: 'contain',
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    })
    .png()
    .toFile(outPath);

  console.log('Wrote', outPath);

  const iconsDir = path.join(root, 'src-tauri', 'icons');
  execSync(`npx --yes @tauri-apps/cli icon "${outPath}" -o "${iconsDir}"`, {
    cwd: root,
    stdio: 'inherit',
    shell: true,
    env: process.env,
  });
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
