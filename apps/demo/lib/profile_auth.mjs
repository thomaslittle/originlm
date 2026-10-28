export function connectionAuthLabel(connection){
 const auth=connection?.auth??{};
 if(auth.state==='known'){
  if(auth.value==='authenticated')return auth.freshness==='stale'?'Previously authenticated':'Authenticated';
  if(auth.value==='unauthenticated')return 'Not authenticated';
  if(auth.value==='expired')return 'Expired';
  if(auth.value==='unchecked')return 'Not checked';
 }
 if(auth.reason==='LOCAL_RUNTIME_HAS_NO_ACCOUNT_AUTH')return 'Local runtime · account auth not applicable';
 if(auth.reason==='OPENCODE_UPSTREAM_CREDENTIALS_NOT_VALIDATED')return 'Credentials configured · authentication not verified';
 if(auth.reason==='NOT_CHECKED'||auth.reason==='NOT_PROBED')return 'Not checked';
 return 'Authentication unknown';
}
