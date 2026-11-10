// Builds ../ui/app.js, ../ui/tailwind.css and copies React into ../ui/vendor.
// Only needed when you edit app.jsx. The built files are committed, so end users never need Node.
import { transformSync } from '@babel/core';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync } from 'node:fs';
import { execSync } from 'node:child_process';

const jsx = readFileSync('app.jsx', 'utf8');
const out = transformSync(jsx, { presets: [['@babel/preset-react', { runtime: 'classic' }]], compact: true, comments: false });
writeFileSync('../ui/app.js', out.code);

mkdirSync('../ui/vendor', { recursive: true });
copyFileSync('node_modules/react/umd/react.production.min.js', '../ui/vendor/react.production.min.js');
copyFileSync('node_modules/react-dom/umd/react-dom.production.min.js', '../ui/vendor/react-dom.production.min.js');

execSync('npx tailwindcss -c tailwind.config.js -i input.css -o ../ui/tailwind.css --minify', { stdio: 'inherit' });
console.log('ui built');
