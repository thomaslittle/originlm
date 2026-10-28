const registeredProfileProviders=['grok','gemini_cli','antigravity'];
const standardProviders=['codex','claude_code','opencode','ollama','lmstudio','openai_api','anthropic_api','gemini_api','xai_api','compatible_api'];

export function liveDemoExecutionArgs(workspace,{nativeProfiles=false}={}){
 if(typeof workspace!=='string'||!workspace.trim())throw Error('LIVE_DEMO_WORKSPACE_REQUIRED');
 const providers=[...standardProviders,...(nativeProfiles?registeredProfileProviders:[])];
 return [...providers.flatMap(provider=>['--allow-execution',provider]),'--workspace',workspace];
}
