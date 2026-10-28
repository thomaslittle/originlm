import {build} from 'esbuild';
import {gzipSync} from 'node:zlib';
const result=await build({entryPoints:[new URL('../packages/sdk-typescript/src/index.ts',import.meta.url).pathname.replace(/^\/([A-Za-z]:)/,'$1')],bundle:true,minify:true,format:'esm',platform:'browser',write:false});
const bytes=result.outputFiles[0].contents;
console.log(JSON.stringify({rawBytes:bytes.length,gzipBytes:gzipSync(bytes).length}));
