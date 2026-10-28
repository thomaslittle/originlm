/** Read the already-existing Kit snapshot only. No discovery, credentials, migrations or retries. */
import {isAbsolute} from 'node:path';
import {DatabaseSync} from 'node:sqlite';

export function createSqliteUsageCache({databasePath,ownerApprovedCacheRead=false,maxSnapshotBytes=8*1024*1024}) {
 if(ownerApprovedCacheRead!==true)throw new Error('OWNER_CACHE_ACCESS_NOT_APPROVED');
 if(typeof databasePath!=='string'||!isAbsolute(databasePath)||!Number.isSafeInteger(maxSnapshotBytes)
  ||maxSnapshotBytes<1024||maxSnapshotBytes>8*1024*1024)throw new Error('INVALID_USAGE_CACHE_CONFIG');
 return {getSnapshot(){
  let db;
  try {
   db=new DatabaseSync(databasePath,{readOnly:true});
   db.exec('PRAGMA query_only=ON; PRAGMA busy_timeout=100;');
   const row=db.prepare('SELECT payload FROM snapshot WHERE singleton=1 AND length(CAST(payload AS BLOB))<=?').get(maxSnapshotBytes);
   if(!row||typeof row.payload!=='string')return {snapshot:null,loading:false,error:null};
   return {snapshot:JSON.parse(row.payload),loading:false,error:null};
  }catch{return {snapshot:null,loading:false,error:null};}
  finally{db?.close();}
 }};
}
