import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { build } from 'esbuild';
import { chromium } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
let browser, script, styles;
before(async()=>{
 const result=await build({stdin:{contents:`
  import React from 'react';
  import {createRoot} from 'react-dom/client';
  import {ProviderModelSelector} from '@originlm/ui-react';
  import {fixtureSnapshot} from './apps/demo/fixture/snapshot.mjs';
  const snapshot=fixtureSnapshot(),first=snapshot.models[0];
  function Fixture({variant}){
   const [value,onValueChange]=React.useState({routeId:first.routeId,modelId:first.id,revision:first.revision,options:{}});
   return React.createElement(ProviderModelSelector,{snapshot,value,onValueChange,variant});
  }
  const root=createRoot(document.querySelector('#root'));
  window.renderFixture=variant=>root.render(React.createElement(Fixture,{variant}));
 `,resolveDir:process.cwd()},bundle:true,write:false,format:'iife',platform:'browser',target:'es2022'});
 script=result.outputFiles[0].text;
 styles=await readFile(new URL('../packages/ui-react/styles.css',import.meta.url),'utf8');
 browser=await chromium.launch({headless:true});
});
after(async()=>{await browser?.close();});
for(const variant of ['rail','compact','palette','capability','compare','allowance','preset','mobile']){
 test(`packaged React ${variant}: no automated WCAG A/AA violations`,async()=>{
  const context=await browser.newContext({viewport:{width:variant==='mobile'?390:1200,height:900}});
  const page=await context.newPage();
  try{
   await page.setContent('<!doctype html><html lang="en"><head><title>Selector package accessibility fixture</title></head><body><main><h1>Reusable AI selector</h1><div id="root"></div></main></body></html>');
   await page.addStyleTag({content:styles});await page.addScriptTag({content:script});
   await page.evaluate(v=>window.renderFixture(v),variant);
   await page.locator('#root button').first().waitFor();
   const result=await new AxeBuilder({page}).withTags(['wcag2a','wcag2aa','wcag21a','wcag21aa']).analyze();
   assert.deepEqual(result.violations.map(v=>({id:v.id,nodes:v.nodes.map(n=>({target:n.target,summary:n.failureSummary}))})),[]);
  }finally{await context.close();}
 });
}
