import {fileURLToPath} from 'node:url';
import {resolve,dirname} from 'node:path';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
// The workspace packages must be built first (`npm run build` does this); the
// React demo bundles them from their dist output. `npm run build:demo` chains
// both steps so the native-served assets are always reproducible.
const {build}=await import('vite');
await build({configFile:resolve(root,'apps/demo/vite.config.ts')});
console.log('Built native-served demo assets: apps/demo/dist');
