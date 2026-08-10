use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use conductor_domain::{ManagedResource, PrimaryRole, ResourceAccessPolicy, User};
use dashmap::{mapref::entry::Entry, DashMap};
use tokio::sync::{broadcast, OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

use crate::core::config::RealtimeConfig;

pub const PROTOCOL_NAME: &str = "evoflux.realtime.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealtimeAudience {
    All,
    Owner(Uuid),
    Policy {
        owner_user_id: Uuid,
        policy: ResourceAccessPolicy,
    },
}

impl RealtimeAudience {
    pub fn includes(&self, user: &User) -> bool {
        match self {
            Self::All => true,
            Self::Owner(owner_user_id) => *owner_user_id == user.id,
            Self::Policy {
                owner_user_id,
                policy,
            } => {
                user.primary_role == PrimaryRole::Admin
                    || *owner_user_id == user.id
                    || policy.all_members
                    || policy
                        .primary_roles
                        .iter()
                        .any(|role| role == user.primary_role.as_str())
                    || policy
                        .sub_role_ids
                        .iter()
                        .any(|id| user.sub_role_ids.contains(id))
                    || policy.tag_ids.iter().any(|id| user.tag_ids.contains(id))
                    || policy.member_ids.contains(&user.id)
            }
        }
    }
}

/// Signals published by resource/application services after a committed change.
/// The local hub is deliberately transport-only; database writes remain the source of truth.
#[derive(Debug, Clone)]
pub enum RealtimeSignal {
    ResourceUpsert {
        audience: RealtimeAudience,
        resource: ManagedResource,
    },
    ResourceDelete {
        audience: RealtimeAudience,
        resource_id: Uuid,
    },
    AccessRevoked {
        secret_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        reason: String,
    },
    ServerDrain {
        retry_after_ms: u64,
    },
}

#[derive(Debug, Clone)]
pub struct RealtimeMessage {
    pub sequence: u64,
    pub emitted_at: DateTime<Utc>,
    pub signal: RealtimeSignal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeCapacityError {
    Global,
    PerSecret,
    Handshake,
}

#[derive(Clone)]
pub struct RealtimeHub {
    sender: broadcast::Sender<RealtimeMessage>,
    sequence: Arc<AtomicU64>,
    global_connections: Arc<Semaphore>,
    concurrent_handshakes: Arc<Semaphore>,
    per_secret_connections: Arc<DashMap<Uuid, Arc<Semaphore>>>,
    owner_connections: Arc<DashMap<Uuid, usize>>,
    config: Arc<RwLock<RealtimeConfig>>,
}

impl RealtimeHub {
    pub fn new(config: RealtimeConfig) -> Self {
        let (sender, _) = broadcast::channel(config.broadcast_capacity);
        Self {
            sender,
            sequence: Arc::new(AtomicU64::new(0)),
            global_connections: Arc::new(Semaphore::new(config.max_connections)),
            concurrent_handshakes: Arc::new(Semaphore::new(config.max_concurrent_handshakes)),
            per_secret_connections: Arc::new(DashMap::new()),
            owner_connections: Arc::new(DashMap::new()),
            config: Arc::new(RwLock::new(config)),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RealtimeMessage> {
        self.sender.subscribe()
    }

    pub fn publish(&self, signal: RealtimeSignal) -> u64 {
        let sequence = self.next_sequence();
        let _ = self.sender.send(RealtimeMessage {
            sequence,
            emitted_at: Utc::now(),
            signal,
        });
        sequence
    }

    pub fn next_sequence(&self) -> u64 {
        self.sequence.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn disconnect_secret(&self, secret_id: Uuid, reason: impl Into<String>) {
        self.publish(RealtimeSignal::AccessRevoked {
            secret_id: Some(secret_id),
            owner_user_id: None,
            reason: reason.into(),
        });
    }

    pub fn disconnect_owner(&self, owner_user_id: Uuid, reason: impl Into<String>) {
        self.publish(RealtimeSignal::AccessRevoked {
            secret_id: None,
            owner_user_id: Some(owner_user_id),
            reason: reason.into(),
        });
    }

    pub fn try_connect(
        &self,
        secret_id: Uuid,
        owner_user_id: Uuid,
    ) -> Result<RealtimeConnectionPermit, RealtimeCapacityError> {
        let global = self
            .global_connections
            .clone()
            .try_acquire_owned()
            .map_err(|_| RealtimeCapacityError::Global)?;

        let secret_semaphore = self
            .per_secret_connections
            .entry(secret_id)
            .or_insert_with(|| {
                Arc::new(Semaphore::new(
                    self.config
                        .read()
                        .expect("realtime config poisoned")
                        .max_connections_per_secret,
                ))
            })
            .clone();
        let secret = secret_semaphore
            .try_acquire_owned()
            .map_err(|_| RealtimeCapacityError::PerSecret)?;

        *self.owner_connections.entry(owner_user_id).or_insert(0) += 1;

        Ok(RealtimeConnectionPermit {
            _global: global,
            _secret: secret,
            owner_user_id,
            owner_connections: self.owner_connections.clone(),
        })
    }

    pub fn try_begin_handshake(&self) -> Result<OwnedSemaphorePermit, RealtimeCapacityError> {
        self.concurrent_handshakes
            .clone()
            .try_acquire_owned()
            .map_err(|_| RealtimeCapacityError::Handshake)
    }

    pub fn active_connections(&self) -> usize {
        self.config
            .read()
            .expect("realtime config poisoned")
            .max_connections
            - self.global_connections.available_permits()
    }

    pub fn active_owners(&self) -> usize {
        self.owner_connections.len()
    }

    pub fn heartbeat_seconds(&self) -> u64 {
        self.config
            .read()
            .expect("realtime config poisoned")
            .heartbeat_seconds
    }

    pub fn config(&self) -> RealtimeConfig {
        self.config
            .read()
            .expect("realtime config poisoned")
            .clone()
    }

    /// Applies operator-tunable limits from the network settings. Raising the
    /// global connection cap takes effect immediately; lowering it fully
    /// applies after restart. Heartbeat and per-secret limits affect new
    /// connections right away. Handshake and broadcast-capacity changes always
    /// require a restart.
    pub fn update_config(&self, next: RealtimeConfig) {
        let mut config = self.config.write().expect("realtime config poisoned");
        if next.max_connections > config.max_connections {
            self.global_connections
                .add_permits(next.max_connections - config.max_connections);
            config.max_connections = next.max_connections;
        }
        config.max_connections_per_secret = next.max_connections_per_secret;
        config.heartbeat_seconds = next.heartbeat_seconds;
    }
}

pub struct RealtimeConnectionPermit {
    _global: OwnedSemaphorePermit,
    _secret: OwnedSemaphorePermit,
    owner_user_id: Uuid,
    owner_connections: Arc<DashMap<Uuid, usize>>,
}

impl Drop for RealtimeConnectionPermit {
    fn drop(&mut self) {
        if let Entry::Occupied(mut entry) = self.owner_connections.entry(self.owner_user_id) {
            if *entry.get() <= 1 {
                entry.remove();
            } else {
                *entry.get_mut() -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> RealtimeConfig {
        RealtimeConfig {
            max_connections: 2,
            max_connections_per_secret: 1,
            max_concurrent_handshakes: 1,
            broadcast_capacity: 8,
            heartbeat_seconds: 20,
        }
    }

    #[test]
    fn enforces_global_and_per_secret_capacity() {
        let hub = RealtimeHub::new(config());
        let owner_a = Uuid::new_v4();
        let owner_b = Uuid::new_v4();
        let secret_a = Uuid::new_v4();
        let secret_b = Uuid::new_v4();
        let secret_c = Uuid::new_v4();

        let first = hub.try_connect(secret_a, owner_a).unwrap();
        assert_eq!(hub.active_connections(), 1);
        assert_eq!(hub.active_owners(), 1);
        assert!(matches!(
            hub.try_connect(secret_a, owner_a),
            Err(RealtimeCapacityError::PerSecret)
        ));

        let second = hub.try_connect(secret_b, owner_b).unwrap();
        assert!(matches!(
            hub.try_connect(secret_c, owner_b),
            Err(RealtimeCapacityError::Global)
        ));
        assert_eq!(hub.active_connections(), 2);
        assert_eq!(hub.active_owners(), 2);

        drop(first);
        drop(second);
        assert_eq!(hub.active_connections(), 0);
        assert_eq!(hub.active_owners(), 0);
    }

    #[test]
    fn bounds_concurrent_handshakes() {
        let hub = RealtimeHub::new(config());
        let first = hub.try_begin_handshake().unwrap();
        assert!(matches!(
            hub.try_begin_handshake(),
            Err(RealtimeCapacityError::Handshake)
        ));
        drop(first);
        assert!(hub.try_begin_handshake().is_ok());
    }

    #[test]
    fn tracks_ten_thousand_connection_permits_and_cleans_presence() {
        let hub = RealtimeHub::new(RealtimeConfig::default());
        let permits: Vec<_> = (0..10_000)
            .map(|_| hub.try_connect(Uuid::new_v4(), Uuid::new_v4()).unwrap())
            .collect();

        assert_eq!(hub.active_connections(), 10_000);
        assert_eq!(hub.active_owners(), 10_000);
        assert!(matches!(
            hub.try_connect(Uuid::new_v4(), Uuid::new_v4()),
            Err(RealtimeCapacityError::Global)
        ));

        drop(permits);
        assert_eq!(hub.active_connections(), 0);
        assert_eq!(hub.active_owners(), 0);
    }

    #[tokio::test]
    async fn publishes_one_ordered_message_to_all_receivers() {
        let hub = RealtimeHub::new(config());
        let mut first = hub.subscribe();
        let mut second = hub.subscribe();

        let sequence = hub.publish(RealtimeSignal::ServerDrain {
            retry_after_ms: 2_000,
        });

        assert_eq!(first.recv().await.unwrap().sequence, sequence);
        assert_eq!(second.recv().await.unwrap().sequence, sequence);
    }
}
