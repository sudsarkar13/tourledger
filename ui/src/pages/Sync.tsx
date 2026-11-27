import { CircleCheck, CloudOff, Copy, MonitorSmartphone, RefreshCw, ScanLine, TriangleAlert, Unlink, Wifi, type LucideIcon } from "lucide-react";
import QRCode from "qrcode";
import { useEffect, useRef, useState } from "react";
import { android, api, cancelScan, isMobile, scanCode, type LocalNetwork } from "../api";
import { Button, Card, EmptyState, IconButton, Input, List, ListRow, PageHeader, Sheet, TextArea, Toggle } from "../components/ui";
import { Form, useForm } from "../forms/useForm";
import { ago, plural } from "../format";
import { useOverlay } from "../overlays";
import { calm } from "../splash";
import { useApp } from "../state";
import type { SyncPeer, SyncStatus } from "../types";

/** While something is happening the page asks far more often than usual. */
const LIVE_MS = 700;
/** Linked devices are contacted every 45 s, so older than this is not "up to date". */
const STALE_MS = 2 * 60 * 1000;

const PHASES = { connecting: "Connecting…", comparing: "Comparing records…", pictures: "Moving bill pictures…" };

const doing = (p: SyncPeer) => (p.phase ? PHASES[p.phase] : "Syncing…");

function movedText(m: NonNullable<SyncPeer["moved"]>): string {
  const parts = [
    m.received && `received ${plural(m.received, "change")}`,
    m.sent && `sent ${m.sent}`,
    m.pictures_received && `${plural(m.pictures_received, "picture")} in`,
    m.pictures_sent && `${plural(m.pictures_sent, "picture")} out`,
  ].filter(Boolean);
  return `Last change ${ago(m.at_ms)}: ${parts.join(", ")}`;
}

interface Summary {
  tone: "" | "good" | "warn" | "off" | "busy";
  icon: LucideIcon;
  title: string;
  detail: string;
}

/** One answer to "is my data across?". */
function summarise(sync: SyncStatus): Summary {
  if (!sync.enabled) return { tone: "off", icon: CloudOff, title: "Sync is off", detail: "Nothing is sent or received." };
  if (!sync.online) {
    return { tone: "warn", icon: TriangleAlert, title: "Not connected", detail: "Turn sync off and on again." };
  }
  if (!sync.peers.length) {
    const detail = sync.unlinked_by
      ? `${sync.unlinked_by.name} unlinked this device ${ago(sync.unlinked_by.at_ms)}.`
      : "Link your phone or computer below.";
    return { tone: "", icon: MonitorSmartphone, title: "Ready to link", detail };
  }
  // Merely dialling is not syncing: an absent device is dialled for 20 s at a time.
  const busy = sync.peers.find((p) => p.busy && p.phase !== "connecting");
  if (busy) return { tone: "busy", icon: RefreshCw, title: `Syncing with ${busy.name}`, detail: doing(busy) };
  const broken = sync.peers.find((p) => p.error && !p.away);
  if (broken) {
    return { tone: "warn", icon: TriangleAlert, title: `Cannot sync with ${broken.name}`, detail: broken.error! };
  }
  const missed = sync.peers.filter((p) => p.away);
  if (missed.length) {
    return {
      tone: "warn",
      icon: TriangleAlert,
      title: missed.length === 1 ? `Waiting for ${missed[0].name}` : `Waiting for ${missed.length} devices`,
      detail: "Your entries are safe here and will be sent when it can be reached.",
    };
  }
  const dialling = sync.peers.find((p) => p.busy);
  if (dialling) return { tone: "busy", icon: RefreshCw, title: `Trying to reach ${dialling.name}`, detail: doing(dialling) };
  // The device heard from longest ago decides: all of them must be across.
  const last = Math.min(...sync.peers.map((p) => p.last_sync_ms));
  if (!last) return { tone: "", icon: RefreshCw, title: "Waiting for the first sync", detail: "Keep both devices open." };
  if (Date.now() - last > STALE_MS) {
    return { tone: "", icon: RefreshCw, title: "Checking…", detail: `Last synced ${ago(last)}.` };
  }
  if (sync.pictures_waiting) {
    return {
      tone: "",
      icon: RefreshCw,
      title: "Entries are in",
      detail: `${plural(sync.pictures_waiting, "bill picture")} still to arrive.`,
    };
  }
  const behind = sync.peers.find((p) => p.more_pictures);
  if (behind) {
    return { tone: "", icon: RefreshCw, title: "Sending bill pictures", detail: `${behind.name} still has some to get.` };
  }
  return { tone: "good", icon: CircleCheck, title: "Up to date", detail: `Synced ${ago(last)}.` };
}

function peerStatus(p: SyncPeer): string {
  // While retrying a device that failed, keep saying why.
  if (p.busy && !(p.error && p.phase === "connecting")) return doing(p);
  const last = p.last_sync_ms ? `Last synced ${ago(p.last_sync_ms)}` : "Not synced yet";
  return p.error ? `${last}. ${p.error}` : last;
}

function PairingCode({ code, expires, onCancel }: { code: string; expires: number; onCancel: () => void }) {
  const { notify, fail } = useApp();
  const [image, setImage] = useState("");
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Black on white in both themes: scanners need the contrast.
    QRCode.toDataURL(code, { margin: 2, width: 480, errorCorrectionLevel: "L" }).then(setImage).catch(fail);
  }, [code, fail]);
  useEffect(() => {
    if (image) box.current?.scrollIntoView({ behavior: calm() ? "auto" : "smooth", block: "nearest" });
  }, [image]);
  const minutes = Math.max(1, Math.ceil((expires - Date.now()) / 60000));
  return (
    <div ref={box}>
      <Card title="Link another device">
        <div className="sync-hero" role="status">
          <span className="orb waiting">
            <MonitorSmartphone size={26} aria-hidden />
          </span>
          <div>
            <strong>Waiting for the other device</strong>
            <small className="muted">
              Code valid for about {minutes} min, for one device only.
            </small>
          </div>
        </div>
        <p>
          On the other device open <strong>More → Sync</strong>, then scan this or paste the code.
        </p>
        {image && <img className="qr" src={image} alt="Pairing QR code" />}
        <code className="pairing">{code}</code>
        <div className="actions">
          <Button icon={Copy} onClick={() => navigator.clipboard.writeText(code).then(() => notify("Code copied"), fail)}>
            Copy code
          </Button>
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
        </div>
        <p className="muted">Anyone holding this code can link to your data until it is used or expires. Do not post it anywhere.</p>
      </Card>
    </div>
  );
}

function Scanner({ onResult, onClose }: { onResult: (code: string) => void; onClose: () => void }) {
  const { fail } = useApp();
  const stop = () => {
    void cancelScan();
    onClose();
  };
  useOverlay(stop);
  useEffect(() => {
    scanCode()
      .then((code) => code && onResult(code))
      .catch((error) => {
        // Cancelling is not a failure; anything else is shown.
        if (!/cancel/i.test(String((error as Error)?.message ?? error))) fail(error);
      })
      .finally(onClose);
    // Starts once; the callbacks are not meant to restart the camera.
  }, []);
  return (
    <div className="scanner">
      <p>Point the camera at the code</p>
      <Button onClick={stop}>Cancel</Button>
    </div>
  );
}

/** Whether Android still has to be asked for the local network. */
function useLocalNetwork() {
  const [state, setState] = useState<LocalNetwork>(() => android?.localNetwork() ?? "not_needed");
  useEffect(() => {
    if (state !== "needed") return;
    // The system dialog reports back nowhere, so look again until it changes.
    const timer = setInterval(() => {
      const now = android?.localNetwork() ?? "not_needed";
      if (now === "needed") return;
      setState(now);
      void api({ op: "sync.wake" }).catch(() => {});
    }, 1000);
    return () => clearInterval(timer);
  }, [state]);
  const ask = () => {
    if (state === "needed") android?.requestLocalNetwork();
  };
  return { needed: state === "needed", ask };
}

export function Sync() {
  const { sync, setSync, reload, notify, fail, confirm } = useApp();
  const [joining, setJoining] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [linking, setLinking] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const form = useForm({ code: "" });
  const [name, setName] = useState<string | null>(null);
  const network = useLocalNetwork();

  const live = linking || syncing || busy === "unlink" || !!sync?.pairing || !!sync?.peers.some((p) => p.busy);
  useEffect(() => {
    if (!live) return;
    const timer = setInterval(() => {
      api<SyncStatus>({ op: "sync.status" }).then(setSync, () => {});
    }, LIVE_MS);
    return () => clearInterval(timer);
  }, [live, setSync]);

  // The device showing the code learns it worked when a new link appears.
  const known = useRef<string[] | null>(null);
  const linkingHere = useRef(false);
  const peers = sync?.peers;
  useEffect(() => {
    if (!peers) return;
    const added = peers.find((p) => known.current && !known.current.includes(p.id));
    if (added && !linkingHere.current) notify(`Linked with ${added.name}`);
    known.current = peers.map((p) => p.id);
  }, [peers, notify]);

  const call = async (what: string, request: Record<string, unknown>, done?: string) => {
    setBusy(what);
    try {
      setSync(await api<SyncStatus>(request));
      await reload();
      if (done) notify(done);
      return true;
    } catch (error) {
      fail(error);
      return false;
    } finally {
      setBusy(null);
    }
  };

  const syncNow = async () => {
    setSyncing(true);
    try {
      const next = await api<SyncStatus>({ op: "sync.now" });
      setSync(next);
      await reload();
      const missed = next.peers.find((p) => p.error);
      if (missed) notify(missed.away ? `${missed.name} could not be reached` : `${missed.name}: ${missed.error}`, "error");
      // One still busy was already syncing; the page shows how far it is. With
      // none left, the sync found this device had been unlinked.
      else if (next.peers.length && !next.peers.some((p) => p.busy)) notify("Up to date");
    } catch (error) {
      fail(error);
    } finally {
      setSyncing(false);
    }
  };

  const unlink = async (peer: SyncPeer) => {
    const yes = await confirm(
      `Unlink ${peer.name}?`,
      "Both devices drop this link and keep the data they have now. If it is not reachable now, it drops the link the next time it connects. Your other devices stay linked to it and can pass your changes on, so unlink it on each of them too.",
      "Unlink",
    );
    if (!yes) return;
    setBusy("unlink");
    try {
      const next = await api<SyncStatus>({ op: "sync.unlink", peer: peer.id });
      setSync(next);
      notify(
        next.told
          ? "Unlinked on both devices"
          : `Unlinked here. ${peer.name} drops the link when it is next open with sync on.`,
      );
    } catch (error) {
      fail(error);
    } finally {
      setBusy(null);
    }
  };

  const join = async (code: string) => {
    network.ask();
    setLinking(true);
    linkingHere.current = true;
    try {
      setSync(await api<SyncStatus>({ op: "sync.join", code }));
      await reload();
      notify("Devices linked");
    } finally {
      setLinking(false);
      // After the new link has been seen, so it is not announced twice.
      setTimeout(() => (linkingHere.current = false), 0);
    }
  };

  const back = { href: "#/more", label: "More" };
  if (!sync) {
    return <PageHeader title="Sync" back={back} />;
  }

  const summary = summarise(sync);
  const SummaryIcon = summary.icon;
  const show = (
    <Button
      variant={isMobile ? "secondary" : "primary"}
      disabled={busy !== null}
      onClick={() => {
        network.ask();
        void call("pair", { op: "sync.pair" });
      }}
    >
      {busy === "pair" ? "Preparing code…" : "Show my code"}
    </Button>
  );

  return (
    <>
      <PageHeader title="Sync" back={back} context="Between your own devices, encrypted, with no account" />
      <Card>
        <div className="sync-hero" role="status">
          <span className={`orb ${summary.tone}`}>
            <SummaryIcon size={26} aria-hidden />
          </span>
          <div>
            <strong>{summary.title}</strong>
            <small className="muted">{summary.detail}</small>
          </div>
        </div>
        <Toggle
          label="Sync with my linked devices"
          hint={sync.enabled ? "Both devices need TourLedger open." : "This device sends and receives nothing."}
          checked={sync.enabled}
          disabled={busy !== null}
          onChange={(enabled) => {
            if (enabled) network.ask();
            void call("enable", { op: "sync.enable", enabled });
          }}
        />
        {sync.error && (
          <p className="error" role="alert">
            {sync.error}
          </p>
        )}
        {sync.enabled && sync.peers.length > 0 && (
          <div className="actions">
            <Button icon={RefreshCw} className={syncing ? "busy" : ""} disabled={syncing} onClick={syncNow}>
              {syncing ? "Syncing…" : "Sync now"}
            </Button>
          </div>
        )}
      </Card>

      {sync.enabled && network.needed && (
        <Card title="Allow Wi-Fi sync">
          <p>
            Android asks before an app may reach other devices on your Wi-Fi. Without this, sync works only through the
            internet.
          </p>
          <div className="actions">
            <Button variant="primary" icon={Wifi} onClick={network.ask}>
              Allow
            </Button>
            <Button variant="ghost" onClick={() => android?.openAppSettings()}>
              Open app settings
            </Button>
          </div>
        </Card>
      )}

      {linking && (
        <Card>
          <div className="sync-hero" role="status">
            <span className="orb busy">
              <RefreshCw size={26} aria-hidden />
            </span>
            <div>
              <strong>Linking…</strong>
              <small className="muted">Reaching the other device. This can take half a minute.</small>
            </div>
          </div>
        </Card>
      )}

      {sync.pairing && (
        <PairingCode
          code={sync.pairing.code}
          expires={sync.pairing.expires_ms}
          onCancel={() => call("cancel", { op: "sync.cancel_pair" })}
        />
      )}

      <Card title="Linked devices">
        {sync.peers.length ? (
          <List>
            {sync.peers.map((p) => (
              <ListRow
                key={p.id}
                icon={MonitorSmartphone}
                title={p.name}
                detail={
                  <>
                    <span>{peerStatus(p)}</span>
                    {p.moved && <span>{movedText(p.moved)}</span>}
                  </>
                }
              >
                <IconButton label={`Unlink ${p.name}`} icon={Unlink} disabled={busy !== null} onClick={() => unlink(p)} />
              </ListRow>
            ))}
          </List>
        ) : (
          <EmptyState icon={MonitorSmartphone} title="No devices linked yet.">
            <ol className="steps muted">
              <li>Open TourLedger on both devices, then More → Sync.</li>
              <li>On the computer, press Show my code.</li>
              <li>On the phone, press Scan a code.</li>
            </ol>
          </EmptyState>
        )}
        <div className="actions">
          {isMobile && (
            <Button variant="primary" icon={ScanLine} disabled={busy !== null || linking} onClick={() => setScanning(true)}>
              Scan a code
            </Button>
          )}
          {show}
          <Button disabled={busy !== null || linking} onClick={() => setJoining(true)}>
            Enter a code
          </Button>
        </div>
      </Card>

      <Card title="This device">
        <form
          className="inline-form"
          onSubmit={(e) => {
            e.preventDefault();
            // Keep what was typed if the save was refused.
            void call("rename", { op: "sync.rename", name: name ?? sync.name }, "Name saved").then(
              (saved) => saved && setName(null),
            );
          }}
        >
          <Input label="Name shown on your other devices" value={name ?? sync.name} onChange={setName} />
          {name !== null && name !== sync.name && (
            <Button type="submit">Save name</Button>
          )}
        </form>
      </Card>

      {joining && (
        <Sheet title="Enter a code" onClose={() => setJoining(false)}>
          <Form
            onSubmit={form.submit(async () => {
              await join(form.values.code);
              setJoining(false);
              form.set("code", "");
            })}
            busy={form.busy}
            message={form.message}
            submitLabel={form.busy ? "Linking…" : "Link devices"}
          >
            <TextArea
              label="Code from the other device"
              value={form.values.code}
              onChange={(x) => form.set("code", x)}
              error={form.errors.code}
              placeholder="TL1.…"
            />
          </Form>
        </Sheet>
      )}
      {scanning && <Scanner onResult={(code) => join(code).catch(fail)} onClose={() => setScanning(false)} />}
    </>
  );
}
