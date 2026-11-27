//! The running app behind a UI: the ledger, this device's identity and, when
//! turned on, live sync with linked devices.

mod config;
mod pairing;
mod protocol;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use data_encoding::BASE64;
use iroh::endpoint::{Connection, RecvStream, SendStream, presets};
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode};
use iroh_mdns_address_lookup::MdnsAddressLookup;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Notify, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{Instant, timeout};
use tourledger_core::blobs::hash_of;
use tourledger_core::hlc::{Stamp, now_ms};
use tourledger_core::share::{self, FORMAT_VERSION};
use tourledger_core::{Error, Record, Result, Store, api};

use config::DeviceConfig;
use pairing::{Code, Offer};
use protocol::{ALPN, Digest, MAX_HELLO, MAX_MESSAGE, Message, net, receive, send, unexpected};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(300);
/// Pictures fetched in one exchange; the rest follow on the next.
const PICTURES_PER_EXCHANGE: usize = 20;
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// How long unlinking waits for the other device to hear of it.
const GOODBYE_TIMEOUT: Duration = Duration::from_secs(8);
/// How often linked devices are contacted when nothing changed here.
const IDLE_INTERVAL: Duration = Duration::from_secs(45);
const TICK: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    /// Reachable across the internet through iroh's discovery and relays.
    Public,
    /// Direct addresses only; nothing is published anywhere.
    Direct,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op")]
enum SyncRequest {
    #[serde(rename = "sync.status")]
    Status,
    #[serde(rename = "sync.enable")]
    Enable { enabled: bool },
    #[serde(rename = "sync.rename")]
    Rename { name: String },
    #[serde(rename = "sync.pair")]
    Pair,
    #[serde(rename = "sync.cancel_pair")]
    CancelPair,
    #[serde(rename = "sync.join")]
    Join { code: String },
    #[serde(rename = "sync.unlink")]
    Unlink { peer: String },
    #[serde(rename = "sync.now")]
    Now,
    /// The app came back to the front or the network changed.
    #[serde(rename = "sync.wake")]
    Wake,
}

struct Running {
    endpoint: Endpoint,
    tasks: Vec<JoinHandle<()>>,
}

/// Where an exchange under way has got to.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Connecting,
    Comparing,
    Pictures,
}

/// What one exchange moved. Only an exchange that moved something is kept.
#[derive(Debug, Clone, Copy, Default, Serialize)]
struct Moved {
    at_ms: u64,
    received: usize,
    sent: usize,
    pictures_received: usize,
    pictures_sent: usize,
}

impl Moved {
    fn anything(&self) -> bool {
        self.received + self.sent + self.pictures_received + self.pictures_sent > 0
    }
}

/// A link another device ended.
#[derive(Debug, Clone, Serialize)]
struct Dropped {
    name: String,
    at_ms: u64,
}

#[derive(Default)]
struct State {
    unlinked_by: Option<Dropped>,
    offer: Option<Offer>,
    /// Latest failure per peer; cleared by a success.
    errors: BTreeMap<EndpointId, String>,
    busy: HashSet<EndpointId>,
    /// Exchanges under way, in either direction.
    phase: BTreeMap<EndpointId, Phase>,
    moved: BTreeMap<EndpointId, Moved>,
    /// What was offered to each peer last time. A record a peer keeps back is
    /// offered every time, and must be counted as sent only once.
    offered: BTreeMap<EndpointId, HashSet<String>>,
    /// Peers that asked for a full batch of pictures, so more are to come.
    more_pictures: BTreeSet<EndpointId>,
    /// Bumped on going offline: what an older dial finds out no longer counts.
    session: u64,
    /// Bumped on waking: an older dial must not put off the next one.
    wakes: u64,
    /// Store revision each peer last received, and when.
    sent: BTreeMap<EndpointId, (u64, Instant)>,
    network_error: Option<String>,
}

pub struct Service {
    dir: PathBuf,
    network: Network,
    store: Mutex<Store>,
    config: Mutex<DeviceConfig>,
    state: Mutex<State>,
    running: tokio::sync::Mutex<Option<Running>>,
    changed: Notify,
}

/// A lock only fails if another thread panicked while holding it. The data it
/// guards is written transactionally, so carrying on is safe.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn digest(store: &Store) -> Digest {
    share::shareable(store)
        .map(|r| (r.kind.clone(), r.id.clone(), r.stamp.clone()))
        .collect()
}

/// Records here that the holder of `theirs` lacks or has an older copy of.
fn newer_than(store: &Store, theirs: &Digest) -> Vec<Record> {
    let theirs: BTreeMap<(&str, &str), &Stamp> = theirs
        .iter()
        .map(|(kind, id, stamp)| ((kind.as_str(), id.as_str()), stamp))
        .collect();
    share::shareable(store)
        .filter(|r| {
            theirs
                .get(&(r.kind.as_str(), r.id.as_str()))
                .is_none_or(|s| r.stamp > **s)
        })
        .cloned()
        .collect()
}

impl Service {
    /// Opens the ledger in `dir`. Call `start` afterwards to resume sync.
    pub fn open(dir: &Path, default_name: &str, network: Network) -> Result<Arc<Self>> {
        let config = DeviceConfig::load_or_create(dir, default_name)?;
        let store = Store::open(dir, &config.device_id)?;
        Ok(Arc::new(Self {
            dir: dir.to_owned(),
            network,
            store: Mutex::new(store),
            config: Mutex::new(config),
            state: Mutex::new(State::default()),
            running: tokio::sync::Mutex::new(None),
            changed: Notify::new(),
        }))
    }

    /// Goes online if sync was left on. A failure is shown in the status rather
    /// than stopping the app, which must work offline.
    pub async fn start(self: &Arc<Self>) {
        if lock(&self.config).sync_enabled
            && let Err(error) = self.go_online().await
        {
            lock(&self.state).network_error = Some(error.to_string());
        }
    }

    pub async fn handle(self: &Arc<Self>, request: Value) -> Result<Value> {
        let op = request.get("op").and_then(Value::as_str).unwrap_or("");
        if !op.starts_with("sync.") {
            let request: api::Request = serde_json::from_value(request)
                .map_err(|e| Error::msg(format!("Unreadable request: {e}")))?;
            let result = {
                let mut store = lock(&self.store);
                let before = store.revision();
                let result = api::handle(&mut store, request);
                if store.revision() != before {
                    self.changed.notify_one();
                }
                result
            };
            return result;
        }

        let request: SyncRequest = serde_json::from_value(request)
            .map_err(|e| Error::msg(format!("Unreadable request: {e}")))?;
        match request {
            SyncRequest::Status => {}
            SyncRequest::Enable { enabled } => self.set_enabled(enabled).await?,
            SyncRequest::Rename { name } => {
                let name = name.trim();
                if name.is_empty() || name.chars().count() > 40 {
                    return Err(Error::invalid("name", "Enter a name up to 40 letters."));
                }
                self.edit_config(|c| c.name = name.to_owned())?;
            }
            SyncRequest::Pair => self.offer_pairing().await?,
            SyncRequest::CancelPair => lock(&self.state).offer = None,
            SyncRequest::Join { code } => self.join(&code).await?,
            SyncRequest::Unlink { peer } => {
                let told = self.unlink(&peer).await?;
                let mut status = self.status().await;
                status["told"] = json!(told);
                return Ok(status);
            }
            SyncRequest::Now => self.sync_all().await,
            SyncRequest::Wake => self.wake().await,
        }
        Ok(self.status().await)
    }

    async fn status(&self) -> Value {
        let running = self.running.lock().await.is_some();
        let config = lock(&self.config).clone();
        let (revision, pictures_waiting) = {
            let store = lock(&self.store);
            let blobs = store.blobs();
            let waiting = share::picture_hashes(share::shareable(&store))
                .iter()
                .filter(|hash| !blobs.has(hash))
                .count();
            (store.revision(), waiting)
        };
        let mut state = lock(&self.state);
        if state
            .offer
            .as_ref()
            .is_some_and(|o| o.expires_ms <= now_ms())
        {
            state.offer = None;
        }
        let peers: Vec<Value> = config
            .peers
            .iter()
            .map(|p| {
                json!({
                    "id": p.id.to_string(),
                    "name": p.name,
                    "last_sync_ms": p.last_sync_ms,
                    "error": state.errors.get(&p.id),
                    // Simply absent, as opposed to refused or broken.
                    "away": state.errors.get(&p.id) == Some(&unreachable_device().to_string()),
                    "more_pictures": state.more_pictures.contains(&p.id),
                    "busy": state.busy.contains(&p.id) || state.phase.contains_key(&p.id),
                    "phase": state.phase.get(&p.id),
                    "moved": state.moved.get(&p.id),
                })
            })
            .collect();
        json!({
            "enabled": config.sync_enabled,
            "online": running,
            "name": config.name,
            "peers": peers,
            "pairing": state.offer.as_ref().map(|o| json!({"code": o.code, "expires_ms": o.expires_ms})),
            "error": state.network_error,
            "revision": revision,
            "pictures_waiting": pictures_waiting,
            "unlinked_by": state.unlinked_by,
        })
    }

    fn edit_config(&self, edit: impl FnOnce(&mut DeviceConfig)) -> Result<()> {
        let mut config = lock(&self.config);
        let mut next = config.clone();
        edit(&mut next);
        next.save(&self.dir)?;
        *config = next;
        Ok(())
    }

    async fn set_enabled(self: &Arc<Self>, enabled: bool) -> Result<()> {
        if enabled {
            self.go_online().await?;
            self.edit_config(|c| c.sync_enabled = true)?;
            self.changed.notify_one();
        } else {
            // Record the choice first: if closing stalls, it still stays off.
            self.edit_config(|c| c.sync_enabled = false)?;
            self.go_offline().await;
        }
        Ok(())
    }

    async fn go_online(self: &Arc<Self>) -> Result<()> {
        let mut running = self.running.lock().await;
        if running.is_some() {
            return Ok(());
        }
        let secret = lock(&self.config).secret_key();
        let builder = match self.network {
            Network::Public => {
                Endpoint::builder(presets::N0).address_lookup(MdnsAddressLookup::builder())
            }
            Network::Direct => Endpoint::builder(presets::Minimal).relay_mode(RelayMode::Disabled),
        };
        let endpoint = builder
            .secret_key(secret)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .map_err(net)?;

        let accept = tokio::spawn(self.clone().accept_loop(endpoint.clone()));
        let periodic = tokio::spawn(self.clone().periodic_loop());
        *running = Some(Running {
            endpoint,
            tasks: vec![accept, periodic],
        });
        lock(&self.state).network_error = None;
        Ok(())
    }

    async fn go_offline(&self) {
        let running = self.running.lock().await.take();
        {
            let mut state = lock(&self.state);
            state.offer = None;
            state.session += 1;
            state.more_pictures.clear();
            state.busy.clear();
            state.phase.clear();
            state.sent.clear();
            state.errors.clear();
            state.network_error = None;
        }
        if let Some(running) = running {
            running.tasks.iter().for_each(JoinHandle::abort);
            // Closing the endpoint ends connections still in progress.
            let _ = timeout(Duration::from_secs(3), running.endpoint.close()).await;
        }
    }

    /// Stops networking without forgetting that sync is on.
    pub async fn shutdown(&self) {
        self.go_offline().await;
    }

    async fn endpoint(&self) -> Result<Endpoint> {
        self.running
            .lock()
            .await
            .as_ref()
            .map(|r| r.endpoint.clone())
            .ok_or_else(|| Error::msg("Turn sync on first."))
    }

    async fn own_addr(&self, endpoint: &Endpoint) -> EndpointAddr {
        if self.network == Network::Public {
            // Waits for a relay so the code works from another network. Without
            // internet it gives up and the local addresses are still included.
            let _ = timeout(Duration::from_secs(6), endpoint.online()).await;
        }
        endpoint.addr()
    }

    async fn offer_pairing(self: &Arc<Self>) -> Result<()> {
        self.set_enabled(true).await?;
        let endpoint = self.endpoint().await?;
        let secret = pairing::new_secret();
        let code = Code {
            addr: self.own_addr(&endpoint).await,
            secret: secret.clone(),
        }
        .encode()?;
        lock(&self.state).offer = Some(Offer {
            secret,
            code,
            expires_ms: now_ms() + pairing::LIFETIME_MS,
            failures: 0,
        });
        Ok(())
    }

    async fn join(self: &Arc<Self>, text: &str) -> Result<()> {
        let code = Code::decode(text)?;
        if code.addr.id == lock(&self.config).secret_key().public() {
            return Err(Error::invalid(
                "code",
                "That is this device's own code. Enter it on the other device.",
            ));
        }
        self.set_enabled(true).await?;
        let id = code.addr.id;
        let (had, was_unlinked) = {
            let config = lock(&self.config);
            (config.peer(&id).is_some(), config.unlinked.contains(&id))
        };
        // Linking it again: it must not be told to drop us while we do.
        if was_unlinked {
            self.edit_config(|c| c.unlinked.retain(|other| *other != id))?;
        }
        let outcome = self.sync_with(id, Some(code.addr), Some(code.secret)).await;
        if was_unlinked && lock(&self.config).peer(&id).is_none() {
            self.edit_config(|c| c.mark_unlinked(id))?;
        }
        // The code is used up by now, so the link must be reported as made. What
        // went wrong afterwards shows on its row and is retried.
        if !had && lock(&self.config).peer(&id).is_some() {
            return Ok(());
        }
        outcome
    }

    /// Ends a link on both devices. True when the other one has heard; if not,
    /// it is told when it next calls.
    async fn unlink(&self, peer: &str) -> Result<bool> {
        let id: EndpointId = peer.parse().map_err(|_| Error::NotFound)?;
        let addr = lock(&self.config).peer(&id).and_then(|p| p.addr.clone());
        // First, so that nothing more crosses whatever happens next.
        self.edit_config(|c| {
            c.peers.retain(|p| p.id != id);
            c.mark_unlinked(id);
        })?;
        self.clear_peer(id);
        let Ok(endpoint) = self.endpoint().await else {
            return Ok(false);
        };
        let target = addr.unwrap_or_else(|| EndpointAddr::new(id));
        let told = timeout(GOODBYE_TIMEOUT, say_goodbye(&endpoint, target)).await;
        Ok(matches!(told, Ok(Ok(()))))
    }

    /// The other device ended the link: end it here too.
    fn dropped_by(&self, id: EndpointId) -> Result<()> {
        let mut name = None;
        self.edit_config(|c| {
            name = c.peer(&id).map(|p| p.name.clone());
            c.peers.retain(|p| p.id != id);
        })?;
        if let Some(name) = name {
            self.clear_peer(id);
            lock(&self.state).unlinked_by = Some(Dropped {
                name,
                at_ms: now_ms(),
            });
        }
        Ok(())
    }

    fn clear_peer(&self, id: EndpointId) {
        let mut state = lock(&self.state);
        state.errors.remove(&id);
        state.sent.remove(&id);
        state.phase.remove(&id);
        state.moved.remove(&id);
        state.offered.remove(&id);
        state.more_pictures.remove(&id);
    }

    /// Phones do not tell the networking library that the network changed, so
    /// the page says so, and every linked device is then contacted at once.
    async fn wake(&self) {
        if let Ok(endpoint) = self.endpoint().await {
            endpoint.network_change().await;
            {
                let mut state = lock(&self.state);
                state.wakes += 1;
                state.sent.clear();
            }
            self.changed.notify_one();
        }
    }

    fn enter(&self, id: EndpointId, phase: Phase) {
        lock(&self.state).phase.insert(id, phase);
    }

    /// Which of `records` this peer is being offered, and how many of them it
    /// was not offered before.
    fn first_offers(&self, id: EndpointId, records: &[Record]) -> (usize, HashSet<String>) {
        let keys: HashSet<String> = records
            .iter()
            .map(|r| {
                let s = &r.stamp;
                format!("{}/{}/{}.{}.{}", r.kind, r.id, s.ms, s.counter, s.device)
            })
            .collect();
        let state = lock(&self.state);
        let new = match state.offered.get(&id) {
            Some(before) => keys.difference(before).count(),
            None => keys.len(),
        };
        (new, keys)
    }

    /// Called once an exchange has gone through.
    fn remember(&self, id: EndpointId, mut moved: Moved, offered: HashSet<String>) {
        let mut state = lock(&self.state);
        state.offered.insert(id, offered);
        if moved.anything() {
            moved.at_ms = now_ms();
            state.moved.insert(id, moved);
        }
    }

    /// Takes the progress label away, unless a dial of our own still owns it.
    fn leave(&self, id: EndpointId) {
        let mut state = lock(&self.state);
        if !state.busy.contains(&id) {
            state.phase.remove(&id);
        }
    }

    async fn sync_all(self: &Arc<Self>) {
        let ids: Vec<EndpointId> = lock(&self.config).peers.iter().map(|p| p.id).collect();
        let tasks: Vec<_> = ids
            .into_iter()
            .map(|id| {
                let service = self.clone();
                // Failures are recorded per peer and shown in the status.
                tokio::spawn(async move { service.sync_with(id, None, None).await })
            })
            .collect();
        for task in tasks {
            let _ = task.await;
        }
    }

    async fn periodic_loop(self: Arc<Self>) {
        loop {
            tokio::select! {
                () = self.changed.notified() => {
                    // Let a burst of edits finish.
                    tokio::time::sleep(Duration::from_millis(800)).await;
                }
                () = tokio::time::sleep(TICK) => {}
            }
            let revision = lock(&self.store).revision();
            let ids: Vec<EndpointId> = lock(&self.config).peers.iter().map(|p| p.id).collect();
            for id in ids {
                let due = {
                    let state = lock(&self.state);
                    state
                        .sent
                        .get(&id)
                        .is_none_or(|(sent, at)| *sent != revision || at.elapsed() >= IDLE_INTERVAL)
                };
                if due {
                    let service = self.clone();
                    tokio::spawn(async move { service.sync_with(id, None, None).await });
                }
            }
        }
    }

    /// Dials one device and exchanges changes. With a `secret`, also links it.
    async fn sync_with(
        self: &Arc<Self>,
        id: EndpointId,
        addr: Option<EndpointAddr>,
        secret: Option<String>,
    ) -> Result<()> {
        // A background dial already running is enough, but not when a new code
        // has to be delivered: that must really be sent.
        let (mine, session, wakes) = {
            let mut state = lock(&self.state);
            (state.busy.insert(id), state.session, state.wakes)
        };
        if !mine && secret.is_none() {
            return Ok(());
        }
        let outcome = self.dial(id, addr, secret).await;
        let revision = lock(&self.store).revision();
        let mut state = lock(&self.state);
        // Sync was switched off while this was under way.
        if state.session != session {
            return outcome;
        }
        if mine {
            state.busy.remove(&id);
        }
        if !state.busy.contains(&id) {
            state.phase.remove(&id);
        }
        // Also after a failure, so an absent device is not retried every tick.
        // A dial begun before waking says nothing about the network now.
        if state.wakes == wakes {
            state.sent.insert(id, (revision, Instant::now()));
        }
        match &outcome {
            Ok(()) => state.errors.remove(&id),
            Err(error) => state.errors.insert(id, error.to_string()),
        };
        outcome
    }

    async fn dial(
        &self,
        id: EndpointId,
        addr: Option<EndpointAddr>,
        secret: Option<String>,
    ) -> Result<()> {
        let endpoint = self.endpoint().await?;
        let (name, saved) = {
            let config = lock(&self.config);
            let peer = config.peer(&id);
            if peer.is_none() && secret.is_none() {
                return Ok(());
            }
            (config.name.clone(), peer.and_then(|p| p.addr.clone()))
        };
        let target = addr.or(saved).unwrap_or_else(|| EndpointAddr::new(id));
        self.enter(id, Phase::Connecting);
        let connection = timeout(CONNECT_TIMEOUT, endpoint.connect(target.clone(), ALPN))
            .await
            .map_err(|_| unreachable_device())?
            .map_err(|_| unreachable_device())?;
        let hello = Message::Hello {
            version: FORMAT_VERSION,
            name,
            secret,
            addr: endpoint.addr(),
        };
        let result = timeout(EXCHANGE_TIMEOUT, self.exchange(&connection, hello, target))
            .await
            .map_err(|_| Error::msg("Sync took too long. It will try again."))?;
        connection.close(0u32.into(), b"done");
        result
    }

    async fn exchange(
        &self,
        connection: &Connection,
        hello: Message,
        target: EndpointAddr,
    ) -> Result<()> {
        let id = connection.remote_id();
        let (mut out, mut input) = connection.open_bi().await.map_err(net)?;
        send(&mut out, &hello).await?;
        let linking = matches!(
            hello,
            Message::Hello {
                secret: Some(_),
                ..
            }
        );
        match receive(&mut input, MAX_HELLO).await? {
            Message::Welcome { name } => {
                if linking {
                    lock(&self.state).unlinked_by = None;
                }
                self.edit_config(|c| {
                    // One locked step: an unlink in flight is not undone.
                    if linking || c.peer(&id).is_some() {
                        c.upsert_peer(id, clip(&name), Some(target));
                    }
                })?;
                if lock(&self.config).peer(&id).is_none() {
                    return Ok(());
                }
            }
            // Only the device itself can answer with its key, so this is its word.
            Message::Refused { unlinked: true, .. } if !linking => return self.dropped_by(id),
            Message::Refused { reason, .. } => return Err(Error::msg(reason)),
            _ => return Err(unexpected()),
        }

        self.enter(id, Phase::Comparing);
        let mut moved = Moved::default();
        let offered;
        let entries = digest(&lock(&self.store));
        send(&mut out, &Message::Digest { entries }).await?;
        let Message::Reply { records, entries } = receive(&mut input, MAX_MESSAGE).await? else {
            return Err(unexpected());
        };
        self.ensure_linked(&id)?;
        let records = {
            let mut store = lock(&self.store);
            moved.received = share::accept(&mut store, records)?;
            newer_than(&store, &entries)
        };
        (moved.sent, offered) = self.first_offers(id, &records);
        send(&mut out, &Message::Records { records }).await?;
        self.enter(id, Phase::Pictures);
        moved.pictures_received = self.fetch_pictures(&id, &mut out, &mut input).await?;
        moved.pictures_sent = self.give_pictures(&id, &mut out, &mut input).await?;
        let Message::Done = receive(&mut input, MAX_HELLO).await? else {
            return Err(unexpected());
        };
        self.remember(id, moved, offered);
        self.edit_config(|c| {
            if let Some(peer) = c.peers.iter_mut().find(|p| p.id == id) {
                peer.last_sync_ms = now_ms();
            }
        })
    }

    /// Asks for the pictures our records show but this device does not hold.
    async fn fetch_pictures(
        &self,
        id: &EndpointId,
        out: &mut SendStream,
        input: &mut RecvStream,
    ) -> Result<usize> {
        let mut wanted: BTreeSet<String> = {
            let store = lock(&self.store);
            let blobs = store.blobs();
            share::picture_hashes(share::shareable(&store))
                .into_iter()
                .filter(|hash| !blobs.has(hash))
                .take(PICTURES_PER_EXCHANGE)
                .collect()
        };
        let hashes = wanted.iter().cloned().collect();
        send(out, &Message::Want { hashes }).await?;
        let mut count = 0;
        loop {
            match receive(input, MAX_MESSAGE).await? {
                Message::Blob { hash, data } => {
                    // Only what was asked for, and only if it is what it says.
                    let bytes = BASE64.decode(data.as_bytes()).map_err(|_| unexpected())?;
                    if !wanted.remove(&hash) || hash_of(&bytes) != hash {
                        return Err(unexpected());
                    }
                    self.ensure_linked(id)?;
                    let mut store = lock(&self.store);
                    store.blobs().put(&bytes)?;
                    store.touch();
                    count += 1;
                }
                Message::Sent => return Ok(count),
                _ => return Err(unexpected()),
            }
        }
    }

    /// Sends the asked-for pictures that a shareable entry shows, and no others.
    async fn give_pictures(
        &self,
        id: &EndpointId,
        out: &mut SendStream,
        input: &mut RecvStream,
    ) -> Result<usize> {
        let Message::Want { hashes } = receive(input, MAX_MESSAGE).await? else {
            return Err(unexpected());
        };
        {
            let mut state = lock(&self.state);
            if hashes.len() >= PICTURES_PER_EXCHANGE {
                state.more_pictures.insert(*id);
            } else {
                state.more_pictures.remove(id);
            }
        }
        let mut count = 0;
        for hash in hashes.into_iter().take(PICTURES_PER_EXCHANGE) {
            self.ensure_linked(id)?;
            let bytes = {
                let store = lock(&self.store);
                share::picture_hashes(share::shareable(&store))
                    .contains(&hash)
                    .then(|| store.blobs().get(&hash))
                    .flatten()
            };
            if let Some(bytes) = bytes {
                let data = BASE64.encode(&bytes);
                send(out, &Message::Blob { hash, data }).await?;
                count += 1;
            }
        }
        send(out, &Message::Sent).await?;
        Ok(count)
    }

    /// Checked right before data moves, so turning sync off or unlinking takes
    /// effect on an exchange already under way.
    fn ensure_linked(&self, id: &EndpointId) -> Result<()> {
        let config = lock(&self.config);
        if config.sync_enabled && config.peer(id).is_some() {
            Ok(())
        } else {
            Err(Error::msg("Sync was turned off."))
        }
    }

    async fn accept_loop(self: Arc<Self>, endpoint: Endpoint) {
        // Bounds the work an unknown caller can cause. Unlinked callers get a
        // small pool of their own so they cannot crowd out linked devices.
        let slots = Arc::new(Semaphore::new(8));
        let strangers = Arc::new(Semaphore::new(2));
        while let Some(incoming) = endpoint.accept().await {
            let Ok(slot) = slots.clone().try_acquire_owned() else {
                incoming.refuse();
                continue;
            };
            let service = self.clone();
            let strangers = strangers.clone();
            tokio::spawn(async move {
                let Ok(Ok(connection)) = timeout(HELLO_TIMEOUT, incoming).await else {
                    return;
                };
                let known = lock(&service.config)
                    .peer(&connection.remote_id())
                    .is_some();
                let _slot = if known {
                    slot
                } else {
                    drop(slot);
                    let Ok(stranger) = strangers.try_acquire_owned() else {
                        return;
                    };
                    stranger
                };
                let _ = timeout(EXCHANGE_TIMEOUT, service.serve(&connection)).await;
            });
        }
    }

    async fn serve(&self, connection: &Connection) -> Result<()> {
        let id = connection.remote_id();
        let (mut out, mut input) = timeout(HELLO_TIMEOUT, connection.accept_bi())
            .await
            .map_err(net)?
            .map_err(net)?;
        let hello = timeout(HELLO_TIMEOUT, receive(&mut input, MAX_HELLO))
            .await
            .map_err(net)??;
        let (version, name, secret, addr) = match hello {
            Message::Hello {
                version,
                name,
                secret,
                addr,
            } => (version, name, secret, addr),
            Message::Unlink => {
                self.dropped_by(id)?;
                send(&mut out, &Message::Done).await?;
                let _ = out.finish();
                let _ = timeout(HELLO_TIMEOUT, connection.closed()).await;
                return Ok(());
            }
            _ => return Err(unexpected()),
        };
        let linking = secret.is_some();

        let refusal = if version != FORMAT_VERSION {
            Some("The two devices run different versions. Update TourLedger on both.".to_owned())
        } else {
            self.admit(id, &name, secret, addr)
                .err()
                .map(|e| e.to_string())
        };
        if let Some(reason) = refusal {
            let unlinked = {
                let config = lock(&self.config);
                !linking && config.sync_enabled && config.unlinked.contains(&id)
            };
            send(&mut out, &Message::Refused { reason, unlinked }).await?;
            let _ = out.finish();
            // Wait so the reason arrives before the connection is dropped.
            let _ = timeout(HELLO_TIMEOUT, connection.closed()).await;
            return Ok(());
        }

        {
            let _progress = Progress { service: self, id };
            self.enter(id, Phase::Comparing);
            self.answer(id, &mut out, &mut input).await?;
        }
        let _ = out.finish();
        let _ = timeout(HELLO_TIMEOUT, connection.closed()).await;
        Ok(())
    }

    /// The exchange with a device that has been let in.
    async fn answer(
        &self,
        id: EndpointId,
        out: &mut SendStream,
        input: &mut RecvStream,
    ) -> Result<()> {
        let mut moved = Moved::default();
        let offered;
        let own_name = lock(&self.config).name.clone();
        send(out, &Message::Welcome { name: own_name }).await?;
        let Message::Digest { entries } = receive(input, MAX_MESSAGE).await? else {
            return Err(unexpected());
        };
        self.ensure_linked(&id)?;
        let reply = {
            let store = lock(&self.store);
            Message::Reply {
                records: newer_than(&store, &entries),
                entries: digest(&store),
            }
        };
        let Message::Reply { records, .. } = &reply else {
            return Err(unexpected());
        };
        (moved.sent, offered) = self.first_offers(id, records);
        send(out, &reply).await?;
        let Message::Records { records } = receive(input, MAX_MESSAGE).await? else {
            return Err(unexpected());
        };
        self.ensure_linked(&id)?;
        moved.received = share::accept(&mut lock(&self.store), records)?;
        self.enter(id, Phase::Pictures);
        moved.pictures_sent = self.give_pictures(&id, out, input).await?;
        moved.pictures_received = self.fetch_pictures(&id, out, input).await?;
        self.edit_config(|c| {
            if let Some(peer) = c.peers.iter_mut().find(|p| p.id == id) {
                peer.last_sync_ms = now_ms();
            }
        })?;
        lock(&self.state).errors.remove(&id);
        self.remember(id, moved, offered);
        send(out, &Message::Done).await
    }

    /// Lets in linked devices, and a new one holding the current pairing secret.
    fn admit(
        &self,
        id: EndpointId,
        name: &str,
        secret: Option<String>,
        addr: EndpointAddr,
    ) -> Result<()> {
        // The address must belong to the key that was proved on connecting.
        let addr = (addr.id == id).then_some(addr);
        if !lock(&self.config).sync_enabled {
            return Err(Error::msg("Sync is off on the other device."));
        }
        if let Some(secret) = secret {
            let mut state = lock(&self.state);
            let Some(offer) = state.offer.as_mut().filter(|o| o.expires_ms > now_ms()) else {
                return Err(Error::msg(
                    "That code has expired. Make a new one on the other device.",
                ));
            };
            if offer.secret != secret {
                offer.failures += 1;
                if offer.failures >= pairing::MAX_ATTEMPTS {
                    state.offer = None;
                }
                return Err(Error::msg("That code is not valid."));
            }
            // One code links one device.
            state.offer = None;
            state.unlinked_by = None;
            drop(state);
            return self.edit_config(|c| c.upsert_peer(id, clip(name), addr));
        }
        let mut linked = false;
        self.edit_config(|c| {
            if c.peer(&id).is_some() {
                c.upsert_peer(id, clip(name), addr);
                linked = true;
            }
        })?;
        if linked {
            Ok(())
        } else {
            Err(Error::msg(
                "The other device is no longer linked to this one.",
            ))
        }
    }
}

/// Shows an exchange as under way for as long as it lives.
struct Progress<'a> {
    service: &'a Service,
    id: EndpointId,
}

impl Drop for Progress<'_> {
    fn drop(&mut self) {
        self.service.leave(self.id);
    }
}

/// Tells a device that this one has unlinked it.
async fn say_goodbye(endpoint: &Endpoint, target: EndpointAddr) -> Result<()> {
    let connection = endpoint.connect(target, ALPN).await.map_err(net)?;
    let (mut out, mut input) = connection.open_bi().await.map_err(net)?;
    send(&mut out, &Message::Unlink).await?;
    let answer = receive(&mut input, MAX_HELLO).await;
    connection.close(0u32.into(), b"done");
    match answer? {
        Message::Done => Ok(()),
        _ => Err(unexpected()),
    }
}

fn clip(name: &str) -> String {
    name.chars().take(40).collect()
}

fn unreachable_device() -> Error {
    Error::msg(
        "Not reachable now. Keep TourLedger open there with sync on, \
         on the same Wi-Fi or with internet on both.",
    )
}
