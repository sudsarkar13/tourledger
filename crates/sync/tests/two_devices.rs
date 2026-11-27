//! Real endpoints talking over this machine's network.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tourledger_sync::{Network, Service};

struct Device {
    service: Arc<Service>,
    _dir: tempfile::TempDir,
}

impl Device {
    async fn new(name: &str, network: Network) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::open(dir.path(), name, network).unwrap();
        service.start().await;
        Device { service, _dir: dir }
    }

    async fn call(&self, request: Value) -> Value {
        self.service
            .handle(request.clone())
            .await
            .unwrap_or_else(|e| {
                panic!("{request}: {e}");
            })
    }

    async fn save(&self, kind: &str, body: Value) -> String {
        self.call(json!({"op": "save", "kind": kind, "body": body}))
            .await["id"]
            .as_str()
            .unwrap()
            .into()
    }

    async fn names(&self) -> Vec<String> {
        let snapshot = self
            .call(json!({"op": "snapshot", "today": "2026-11-27"}))
            .await;
        snapshot["people"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap().to_owned())
            .collect()
    }

    async fn person(&self, name: &str) -> String {
        self.save("person", json!({"name": name})).await
    }

    async fn pairing_code(&self) -> String {
        self.call(json!({"op": "sync.pair"})).await["pairing"]["code"]
            .as_str()
            .unwrap()
            .into()
    }

    async fn join(&self, code: &str) -> Result<Value, String> {
        self.service
            .handle(json!({"op": "sync.join", "code": code}))
            .await
            .map_err(|e| e.to_string())
    }

    async fn status(&self) -> Value {
        self.call(json!({"op": "sync.status"})).await
    }

    async fn peers(&self) -> Vec<Value> {
        self.status().await["peers"].as_array().unwrap().clone()
    }
}

async fn unlink_first(device: &Device) -> Value {
    let id = device.peers().await[0]["id"].as_str().unwrap().to_owned();
    device.call(json!({"op": "sync.unlink", "peer": id})).await
}

/// Syncs until `wanted` holds or fails after a few seconds. Background syncs may
/// already be running, so one request is not always enough.
async fn until(from: &Device, to: &Device, wanted: &[&str]) {
    for _ in 0..60 {
        from.call(json!({"op": "sync.now"})).await;
        if to.names().await == wanted {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(to.names().await, wanted);
}

#[tokio::test(flavor = "multi_thread")]
async fn link_merge_cut_off_and_rejoin() {
    let laptop = Device::new("Laptop", Network::Direct).await;
    let phone = Device::new("Phone", Network::Direct).await;
    laptop.person("Asha").await;
    phone.person("Ravi").await;

    let code = laptop.pairing_code().await;
    phone.join(&code).await.unwrap();
    until(&phone, &laptop, &["Asha", "Ravi"]).await;
    until(&laptop, &phone, &["Asha", "Ravi"]).await;
    // Each side can say what it got, and that it is not stuck part way.
    for device in [&laptop, &phone] {
        let peer = &device.peers().await[0];
        assert!(peer["moved"]["received"].as_u64().unwrap() >= 1, "{peer}");
        assert!(peer["moved"]["sent"].as_u64().unwrap() >= 1, "{peer}");
        device.call(json!({"op": "sync.wake"})).await;
    }
    assert_eq!(laptop.peers().await[0]["name"], "Phone");
    assert_eq!(phone.peers().await[0]["name"], "Laptop");

    // A code links one device only.
    let stranger = Device::new("Stranger", Network::Direct).await;
    assert!(stranger.join(&code).await.unwrap_err().contains("expired"));
    assert!(stranger.names().await.is_empty());

    // Turned off, the two drift apart.
    phone
        .call(json!({"op": "sync.enable", "enabled": false}))
        .await;
    laptop.person("Bina").await;
    phone.person("Chetan").await;
    laptop.call(json!({"op": "sync.now"})).await;
    assert_eq!(laptop.names().await, ["Asha", "Bina", "Ravi"]);
    assert_eq!(phone.names().await, ["Asha", "Chetan", "Ravi"]);
    // A device that is simply not there is "away", with nothing left under way.
    let mut peer = laptop.peers().await[0].clone();
    for _ in 0..300 {
        if peer["away"] == true && peer["busy"] == false {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        peer = laptop.peers().await[0].clone();
    }
    assert_eq!(peer["away"], true, "{peer}");
    assert_eq!(peer["phase"], Value::Null, "{peer}");

    // Turned on again, both sides' additions arrive.
    phone
        .call(json!({"op": "sync.enable", "enabled": true}))
        .await;
    until(&phone, &laptop, &["Asha", "Bina", "Chetan", "Ravi"]).await;
    until(&phone, &phone, &["Asha", "Bina", "Chetan", "Ravi"]).await;

    // Unlinking on one ends the link on both, and nothing more crosses.
    let reply = unlink_first(&laptop).await;
    assert_eq!(reply["told"], true, "{reply}");
    assert!(laptop.peers().await.is_empty());
    assert!(phone.peers().await.is_empty());
    assert_eq!(phone.status().await["unlinked_by"]["name"], "Laptop");
    laptop.person("Divya").await;
    phone.person("Esha").await;
    phone.call(json!({"op": "sync.now"})).await;
    assert!(!laptop.names().await.contains(&"Esha".to_owned()));
    assert!(!phone.names().await.contains(&"Divya".to_owned()));

    // Linked again, the pair stays linked.
    let code = laptop.pairing_code().await;
    phone.join(&code).await.unwrap();
    until(
        &phone,
        &laptop,
        &["Asha", "Bina", "Chetan", "Divya", "Esha", "Ravi"],
    )
    .await;
    laptop.call(json!({"op": "sync.now"})).await;
    assert_eq!(laptop.peers().await.len(), 1);
    assert_eq!(phone.peers().await.len(), 1);
    assert_eq!(phone.status().await["unlinked_by"], Value::Null);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_that_was_away_drops_the_link_when_it_returns() {
    let laptop = Device::new("Laptop", Network::Direct).await;
    let phone = Device::new("Phone", Network::Direct).await;
    laptop.person("Asha").await;
    let code = laptop.pairing_code().await;
    phone.join(&code).await.unwrap();
    until(&phone, &phone, &["Asha"]).await;

    phone
        .call(json!({"op": "sync.enable", "enabled": false}))
        .await;
    let reply = unlink_first(&laptop).await;
    assert_eq!(reply["told"], false, "{reply}");
    assert_eq!(phone.peers().await.len(), 1);

    phone
        .call(json!({"op": "sync.enable", "enabled": true}))
        .await;
    for _ in 0..100 {
        if phone.peers().await.is_empty() {
            break;
        }
        phone.call(json!({"op": "sync.now"})).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(phone.peers().await.is_empty());
    assert_eq!(phone.status().await["unlinked_by"]["name"], "Laptop");
}

#[tokio::test(flavor = "multi_thread")]
async fn deletions_travel_and_device_only_tours_stay() {
    let laptop = Device::new("Laptop", Network::Direct).await;
    let phone = Device::new("Phone", Network::Direct).await;
    let gone = laptop.person("Gone").await;
    laptop
        .save(
            "tour",
            json!({"name": "Private", "start_date": "2026-11-01",
                   "end_date": "2026-11-02", "local_only": true}),
        )
        .await;
    let code = laptop.pairing_code().await;
    phone.join(&code).await.unwrap();
    until(&laptop, &phone, &["Gone"]).await;

    phone
        .call(json!({"op": "delete", "kind": "person", "id": gone}))
        .await;
    until(&phone, &laptop, &[]).await;

    let snapshot = phone
        .call(json!({"op": "snapshot", "today": "2026-11-27"}))
        .await;
    assert!(snapshot["tours"].as_array().unwrap().is_empty());
}

/// A small real JPEG (8x8 grey), base64.
const PICTURE: &str = "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////wAALCAAIAAgBAREA/8QAFAABAAAAAAAAAAAAAAAAAAAAB//EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAD8AN//Z";

#[tokio::test(flavor = "multi_thread")]
async fn bill_pictures_travel_both_ways() {
    let laptop = Device::new("Laptop", Network::Direct).await;
    let phone = Device::new("Phone", Network::Direct).await;
    let mut ids = Vec::new();
    for device in [&laptop, &phone] {
        let expense = device
            .save(
                "expense",
                json!({"date": "2026-11-14", "category": "food", "amount": 100, "paid_by": "me"}),
            )
            .await;
        let bill = device
            .call(json!({"op": "add_attachment", "expense_id": expense, "data": PICTURE}))
            .await;
        ids.push(bill["id"].as_str().unwrap().to_owned());
    }
    let code = laptop.pairing_code().await;
    phone.join(&code).await.unwrap();

    // Each device must be able to show the picture the other one took.
    for (device, id) in [(&laptop, &ids[1]), (&phone, &ids[0])] {
        let mut got = None;
        for _ in 0..60 {
            phone.call(json!({"op": "sync.now"})).await;
            got = device
                .service
                .handle(json!({"op": "get_attachment", "id": id}))
                .await
                .ok();
            if got.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert_eq!(got.expect("picture arrived")["data"], PICTURE);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_nonsense_codes_and_its_own() {
    let laptop = Device::new("Laptop", Network::Direct).await;
    assert!(laptop.join("hello").await.is_err());
    let own = laptop.pairing_code().await;
    assert!(laptop.join(&own).await.unwrap_err().contains("own code"));
}

/// Uses iroh's public discovery and relays; run by hand:
/// `cargo test -p tourledger-sync -- --ignored`
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the internet"]
async fn links_through_the_public_network() {
    let laptop = Device::new("Laptop", Network::Public).await;
    let phone = Device::new("Phone", Network::Public).await;
    laptop.person("Asha").await;
    let code = laptop.pairing_code().await;
    phone.join(&code).await.unwrap();
    until(&laptop, &phone, &["Asha"]).await;
}
