import {build} from 'esbuild';
import {fileURLToPath} from 'node:url';
import {cp,mkdir} from 'node:fs/promises';
const root=new URL('../',import.meta.url),src=new URL('examples/browser-extension/',root),out=new URL('dist/browser-extension/',root);
await mkdir(out,{recursive:true});for(const name of ['manifest.json','background.js','index.html','style.css'])await cp(new URL(name,src),new URL(name,out));
await build({entryPoints:[fileURLToPath(new URL('app.ts',src))],outfile:fileURLToPath(new URL('app.js',out)),bundle:true,format:'esm',platform:'browser',target:'es2022'});
console.log('Built generic browser extension in dist/browser-extension (not installed).');
