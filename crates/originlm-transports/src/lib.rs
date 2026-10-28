//! Bounded, handshaken JSON-RPC over native streams. Caller identity is supplied by the host.
pub mod http;
pub mod local_ipc;
pub mod native_messaging;
mod wire;
use originlm_contracts::{ErrorCode, Result};
use originlm_service::NativeService;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    task::JoinSet,
};
use wire::{error, reply};

#[derive(Clone)]
pub struct StdioOptions {
    pub max_frame_bytes: usize,
    pub max_pending_requests: usize,
    pub write_timeout: Duration,
}
impl Default for StdioOptions {
    fn default() -> Self {
        Self {
            // Sized for one staged base64 image (inputs.put); responses remain service-bounded.
            max_frame_bytes: originlm_contracts::MAX_REQUEST_FRAME_BYTES,
            max_pending_requests: 64,
            write_timeout: Duration::from_secs(5),
        }
    }
}
async fn send<W: AsyncWrite + Unpin>(
    writer: &mut W,
    value: Value,
    options: &StdioOptions,
) -> Result<()> {
    let mut bytes = serde_json::to_vec(&value).map_err(|_| error(ErrorCode::InvalidRequest))?;
    if bytes.len() + 1 > options.max_frame_bytes {
        bytes = serde_json::to_vec(&reply(
            value.get("id").cloned().unwrap_or(Value::Null),
            Err(error(ErrorCode::FrameTooLarge)),
        ))
        .map_err(|_| error(ErrorCode::InvalidRequest))?;
    }
    bytes.push(b'\n');
    tokio::time::timeout(options.write_timeout, async {
        writer.write_all(&bytes).await?;
        writer.flush().await
    })
    .await
    .map_err(|_| error(ErrorCode::Timeout))?
    .map_err(|_| error(ErrorCode::ProviderUnavailable))
}

pub async fn serve_io<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    mut reader: R,
    mut writer: W,
    service: Arc<NativeService>,
    client: String,
    options: StdioOptions,
) -> Result<()> {
    if !(1024..=16_777_216).contains(&options.max_frame_bytes)
        || !(1..=256).contains(&options.max_pending_requests)
        || options.write_timeout.is_zero()
        || options.write_timeout > Duration::from_secs(60)
    {
        return Err(error(ErrorCode::InvalidRequest));
    }
    let _lifetime = ClientLifetime {
        service: service.clone(),
        client: client.clone(),
    };
    let mut subscriptions: BTreeMap<String, Value> = BTreeMap::new();
    let mut subscription_sequence = 0_u64;
    let mut ticks = tokio::time::interval(Duration::from_millis(50));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8192];
    let mut ready = false;
    let mut eof = false;
    let mut pending = BTreeSet::new();
    let mut tasks: JoinSet<(String, Value, Result<Value>)> = JoinSet::new();
    while !eof || !tasks.is_empty() {
        tokio::select! {
            biased;
            joined = tasks.join_next(), if !tasks.is_empty() => {
                let (key, id, outcome) = joined.ok_or_else(|| error(ErrorCode::ProviderUnavailable))?.map_err(|_| error(ErrorCode::ProviderUnavailable))?;
                pending.remove(&key);
                send(&mut writer, reply(id, outcome), &options).await?;
            }
            _=ticks.tick(), if !subscriptions.is_empty()&&!eof => {
                for (id,query) in subscriptions.clone() {
                    match service.handle(&client,"events.read",query).await {
                        Ok(page)=>{
                            if let Some(events)=page["events"].as_array(){for event in events {
                                send(&mut writer,serde_json::json!({"jsonrpc":"2.0","method":"events.event","params":{"subscriptionId":id,"event":event}}),&options).await?;
                            }}
                            if let Some(query)=subscriptions.get_mut(&id){query["after"]=page["cursor"].clone();}
                        },
                        Err(problem)=>{
                            subscriptions.remove(&id);
                            send(&mut writer,serde_json::json!({"jsonrpc":"2.0","method":"events.error","params":{"subscriptionId":id,"code":problem.code}}),&options).await?;
                        }
                    }
                }
            }
            read = reader.read(&mut chunk), if !eof => {
                let count = read.map_err(|_| error(ErrorCode::ProviderUnavailable))?;
                if count == 0 {
                    eof = true;
                    if !buffer.is_empty() { send(&mut writer, wire::invalid(-32700), &options).await?; buffer.clear(); }
                    continue;
                }
                for byte in &chunk[..count] {
                    if *byte != b'\n' {
                        if buffer.len() >= options.max_frame_bytes {
                            send(&mut writer, reply(Value::Null, Err(error(ErrorCode::FrameTooLarge))), &options).await?;
                            return Err(error(ErrorCode::FrameTooLarge));
                        }
                        buffer.push(*byte); continue;
                    }
                    if buffer.is_empty() { continue; }
                    let request = wire::parse(&buffer); buffer.clear();
                    let request = match request {
                        Ok(Some(request)) => request,
                        Ok(None) => continue, // Notifications never initiate work in this protocol.
                        Err(value) => { send(&mut writer, value, &options).await?; continue; }
                    };
                    if request.method == "runtime.handshake" {
                        if ready { send(&mut writer, reply(request.id, Err(error(ErrorCode::Conflict))), &options).await?; continue; }
                        let outcome = service.handle(&client, &request.method, request.params).await;
                        ready = outcome.is_ok(); send(&mut writer, reply(request.id, outcome), &options).await?; continue;
                    }
                    if !ready { send(&mut writer, reply(request.id, Err(error(ErrorCode::ProtocolIncompatible))), &options).await?; continue; }
                    if request.method=="events.subscribe" {
                        if subscriptions.len()>=16 {send(&mut writer,reply(request.id,Err(error(ErrorCode::RateLimited))),&options).await?;continue;}
                        match service.handle(&client,"events.read",request.params.clone()).await {
                            Ok(page)=>{
                                subscription_sequence+=1;
                                let id=format!("subscription-{subscription_sequence}");let mut query=request.params;
                                if query.get("after").is_none(){query["after"]=page["tailCursor"].clone();}
                                subscriptions.insert(id.clone(),query);
                                send(&mut writer,reply(request.id,Ok(serde_json::json!({"subscriptionId":id}))),&options).await?;
                            },Err(e)=>send(&mut writer,reply(request.id,Err(e)),&options).await?
                        }
                        continue;
                    }
                    if request.method=="events.unsubscribe" {
                        let result=if request.params.as_object().is_some_and(|p|p.len()==1){request.params["subscriptionId"].as_str().and_then(|id|subscriptions.remove(id)).map(|_|serde_json::json!({"unsubscribed":true})).ok_or_else(||error(ErrorCode::NotFound))}else{Err(error(ErrorCode::InvalidRequest))};
                        send(&mut writer,reply(request.id,result),&options).await?;continue;
                    }
                    let key = request.id.to_string();
                    if pending.contains(&key) {
                        send(&mut writer, reply(request.id, Err(error(ErrorCode::Conflict))), &options).await?;
                        return Err(error(ErrorCode::Conflict));
                    }
                    if pending.len() >= options.max_pending_requests {
                        send(&mut writer, reply(request.id, Err(error(ErrorCode::RateLimited))), &options).await?; continue;
                    }
                    pending.insert(key.clone());
                    let service = service.clone(); let client = client.clone();
                    tasks.spawn(async move { let outcome = service.handle(&client, &request.method, request.params).await; (key, request.id, outcome) });
                }
            }
        }
    }
    Ok(())
}

struct ClientLifetime {
    service: Arc<NativeService>,
    client: String,
}
impl Drop for ClientLifetime {
    fn drop(&mut self) {
        self.service.close_client(&self.client);
    }
}
