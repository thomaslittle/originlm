//! Host-authorized, bounded background work. No provider operation is inferred
//! from UI interest; interest changes cadence only. All deadlines are monotonic.
use crate::NativeService;
use std::{
    collections::BTreeSet,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::{task::JoinSet, time::Instant};

impl NativeService {
    fn interested_clients(&self) -> bool {
        let Ok(clients) = self.clients.read() else {
            return false;
        };
        self.client_interest.lock().is_ok_and(|mut interest| {
            interest.retain(|id, at| {
                clients.contains_key(id) && at.elapsed() < Duration::from_secs(120)
            });
            !interest.is_empty()
        })
    }

    pub(crate) async fn run_refresh_lanes(self: Arc<Self>, generation: u64) {
        let mut probes = JoinSet::new();
        let mut scans: JoinSet<originlm_contracts::Result<()>> = JoinSet::new();
        let mut in_flight = BTreeSet::new();
        let mut last_inventory: Option<Instant> = None;
        let mut metadata_after = String::new();
        let mut usage_after = String::new();
        let mut prefer_usage = false;
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            while let Some(result) = probes.try_join_next() {
                if let Ok(id) = result {
                    in_flight.remove(&id);
                }
            }
            while let Some(result) = scans.try_join_next() {
                if matches!(result, Ok(Err(ref e)) if e.code == originlm_contracts::ErrorCode::Cancelled)
                {
                    last_inventory = None;
                }
            }
            let _ = self.cache.expire_observations();
            let active = self.interested_clients();
            let inventory_period = Duration::from_secs(if active { 300 } else { 1800 });
            if self.inventory_refresh.load(Ordering::SeqCst)
                && scans.is_empty()
                && last_inventory.is_none_or(|at| at.elapsed() >= inventory_period)
            {
                last_inventory = Some(Instant::now());
                let cache = self.cache.clone();
                scans.spawn(async move { cache.refresh_background(generation).await });
            }
            let providers = self
                .refresh_providers
                .read()
                .map(|p| p.clone())
                .unwrap_or_default();
            if providers.is_empty() || self.metadata_source.is_none() {
                continue;
            }
            // Targeted invalidation/unknown metadata is checked every tick;
            // expiry and usage have independent, interest-sensitive deadlines.
            let metadata_period = Duration::from_secs(if active { 900 } else { 3600 });
            let usage_period = Duration::from_secs(if active { 60 } else { 300 });
            let metadata = self
                .cache
                .due_probes(&providers, 64, metadata_period, &metadata_after)
                .unwrap_or_default();
            let usage = self
                .cache
                .usage_due_probes(&providers, 64, usage_period, &usage_after)
                .unwrap_or_default();
            let mut metadata = metadata.into_iter();
            let mut usage = usage.into_iter();
            prefer_usage = !prefer_usage;
            let mut usage_turn = prefer_usage;
            loop {
                if probes.len() >= 2 {
                    break;
                }
                let picked = if usage_turn {
                    usage
                        .next()
                        .map(|t| (t, true))
                        .or_else(|| metadata.next().map(|t| (t, false)))
                } else {
                    metadata
                        .next()
                        .map(|t| (t, false))
                        .or_else(|| usage.next().map(|t| (t, true)))
                };
                let Some((target, is_usage)) = picked else {
                    break;
                };
                if is_usage {
                    usage_after = target.connection_id.clone();
                } else {
                    metadata_after = target.connection_id.clone();
                }
                usage_turn = !usage_turn;
                if !in_flight.insert(target.connection_id.clone()) {
                    continue;
                }
                let service = self.clone();
                probes.spawn(async move {
                    let id = target.connection_id.clone();
                    service
                        .refresh_due_target(target.for_background(generation))
                        .await;
                    id
                });
            }
        }
        // Dropping the JoinSets aborts only this supervisor's async work.
    }
}
