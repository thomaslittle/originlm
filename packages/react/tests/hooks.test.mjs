import test from 'node:test';import assert from 'node:assert/strict';
import React,{act} from 'react';import {renderToString} from 'react-dom/server';import {JSDOM} from 'jsdom';
import {createSnapshotStore} from 'originlm';
import {LocalAIProvider,useLocalAIStore,useLocalAISnapshot} from '../dist/index.js';
const dom=new JSDOM('<!doctype html><html><body></body></html>',{url:'http://127.0.0.1/'});
globalThis.window=dom.window;globalThis.document=dom.window.document;globalThis.HTMLElement=dom.window.HTMLElement;globalThis.IS_REACT_ACT_ENVIRONMENT=true;
const {createRoot}=await import('react-dom/client');
const h=React.createElement;
function snapshot(){return{protocolVersion:'1.0',revision:'one',observedAt:'2026-09-24T00:00:00Z',mode:'synthetic',providers:[],installations:[],profiles:[],accounts:[],connections:[],routes:[],models:[],quotaPools:[],diagnostics:[]};}
function fixture(){let calls=0;const client={snapshot:{get:async()=>{calls++;return snapshot();}},discovery:{refresh:async()=>{calls++;}},connections:{probe:async()=>{calls++;}}};return{store:createSnapshotStore(client),calls:()=>calls};}
function View(){const store=useLocalAIStore(),state=useLocalAISnapshot();return h('div',{},h('output',{},state.snapshot?.revision??'not-loaded'),h('button',{onClick:()=>store.load()},'Load'));}
test('server rendering does not read an account or start a request',()=>{const f=fixture();const html=renderToString(h(LocalAIProvider,{store:f.store},h(View)));assert.ok(html.includes('not-loaded'));assert.equal(f.calls(),0);f.store.dispose();});
test('mounted hooks update only after an explicit public SDK action',async()=>{const f=fixture();const element=document.createElement('div');document.body.append(element);const root=createRoot(element);await act(async()=>root.render(h(LocalAIProvider,{store:f.store},h(View))));assert.equal(f.calls(),0);assert.equal(element.querySelector('output').textContent,'not-loaded');await act(async()=>f.store.load());assert.equal(element.querySelector('output').textContent,'one');assert.equal(f.calls(),1);await act(async()=>root.unmount());f.store.dispose();element.remove();});
test('rerenders and subscription teardown never trigger scans',async()=>{const f=fixture();const element=document.createElement('div');document.body.append(element);const root=createRoot(element);for(let i=0;i<5;i++)await act(async()=>root.render(h(LocalAIProvider,{store:f.store},h(View))));assert.equal(f.calls(),0);await act(async()=>root.unmount());await f.store.load();assert.equal(f.calls(),1);f.store.dispose();element.remove();});
test('hooks require an explicit provider rather than discovering a machine on their own',()=>{assert.throws(()=>renderToString(h(View)),/LocalAIProvider/);});
