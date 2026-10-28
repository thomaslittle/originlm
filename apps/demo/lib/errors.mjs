const messages={
 LOCAL_ACCESS_DENIED:'The host has not granted this provider the local access required for the request. Check its permission and workspace settings.',
 CAPABILITY_UNSUPPORTED:'This provider cannot perform that operation in the current runtime.',
 RUNTIME_MISSING:'A compatible native provider runtime is not installed or registered.',
 INTEGRATION_RESTRICTED:'This provider integration is not configured for this profile.',
 PROVIDER_UNAVAILABLE:'The provider runtime is unavailable. Check that the native client is running and signed in, then retry.',
 AUTH_REQUIRED:'This profile needs authentication in its native provider client.',
 KEYRING_LOCKED:'The operating system credential store is locked. Unlock it, then check the profile again.',
 PROTOCOL_INCOMPATIBLE:'The installed provider client returned an unsupported protocol response. Update or restart the native client, then retry.',
 CAPABILITY_UNKNOWN:'The runtime could not confirm that this provider supports the requested operation.',
 OPTION_UNSUPPORTED:'The selected model no longer accepts one of its configured options. Review the model options and retry.',
 SELECTION_STALE:'The selected account or model changed before the turn started. Refresh the catalog and select it again.',
 RATE_LIMITED:'The provider rate limit has been reached. Wait for it to reset or choose another eligible route.',
 BILLING_CONSENT_REQUIRED:'The selected route requires billing consent. Review the route and explicitly allow paid use before retrying.',
 APPROVAL_REQUIRED:'The provider requested an approval that the runtime could not complete. Retry and respond to the approval prompt.',
 SESSION_ROUTE_MISMATCH:'The session is bound to a different account or route. Start a new session from the current selection.',
 EVENT_GAP:'Some session events were missed. Start a new session to get a complete result.',
 TIMEOUT:'The provider did not finish before the runtime timeout. Check the native client and retry.',
 CANCELLED:'The turn was cancelled before it completed.',
 CONFLICT:'The session is already running or cannot accept this operation in its current state.',
 FRAME_TOO_LARGE:'The provider response exceeded the runtime size limit and was stopped.',
 NOT_FOUND:'The requested session or provider resource is no longer available. Refresh the catalog and retry.',
 NOT_IMPLEMENTED:'This runtime does not implement the requested operation.',
 RESUME_UNAVAILABLE:'This conversation can no longer be continued in its native client. Start a new session.',
 INVALID_REQUEST:'The runtime rejected the request. Review the selected model options and prompt, then retry.',
};

export function userFacingError(error){
 return messages[error?.code]??'The request could not be completed.';
}

/** Use only the normalized protocol code. Provider error text is intentionally redacted. */
export function userFacingSessionError(payload){
 const code=typeof payload?.code==='string'?payload.code:'';
 const explanation=code==='PROVIDER_UNAVAILABLE'
  ?'The provider client stopped or returned an unavailable response. Check that the native client is running and signed in, then retry.'
  :messages[code];
 return explanation?`${explanation} Error code: ${code}.`:'The session failed. The runtime did not provide a recognized error code.';
}

export function debugSessionError(payload={}){
 const code=typeof payload.code==='string'&&/^[A-Z0-9_]{1,64}$/.test(payload.code)?payload.code:'UNKNOWN';
 let message=typeof payload.message==='string'?payload.message:'No diagnostic message was provided.';
 message=message.replace(/\u001b\[[0-9;]*m/g,'')
  .replace(/\b(authorization\s*[:=]\s*)(bearer\s+|basic\s+)?([^\s,;\]}"']+)/gi,(_,prefix,scheme='')=>`${prefix}${scheme}[REDACTED]`)
  .replace(/\b(bearer|basic)\s+([A-Za-z0-9._~+/=-]{8,})/gi,'$1 [REDACTED]')
  .replace(/\b((?:access[_-]?token|refresh[_-]?token|id[_-]?token|client[_-]?secret|api[_-]?key|apikey|password|session[_ -]?id|token|secret)\s*["']?\s*[:=]\s*)(["']?)[^\s,"';}\]]+/gi,'$1$2[REDACTED]')
  .replace(/[\u0000-\u001f\u007f]/g,' ').trim();
 message=[...message].slice(0,2048).join('')||'No diagnostic message was provided.';
 return {code,message};
}
