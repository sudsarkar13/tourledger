// More → Updates, and the question asked when an update is ready.
import { Download, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { Button, Card, PageHeader, Sheet, Toggle } from "../components/ui";
import { usePreferences } from "../preferences";
import { checkForUpdate, downloadUpdate, installUpdate, useUpdate } from "../updates";
import { busy, describe, percent, type UpdateStatus } from "../updateSteps";

const android = () => "TourLedgerUpdates" in window;

const installLabel = () => (android() ? "Install" : "Restart and update");

function Progress({ status }: { status: UpdateStatus }) {
  const done = percent(status) ?? 0;
  return (
    <div className="meter ok">
      <span className="track" role="progressbar" aria-label="Download" aria-valuemin={0} aria-valuemax={100} aria-valuenow={done}>
        <span className="fill" style={{ transform: `scaleX(${done / 100})` }} />
      </span>
    </div>
  );
}

export function Updates() {
  const status = useUpdate();
  const { preferences, update } = usePreferences();
  const header = (
    <PageHeader title="Updates" back={{ href: "#/more", label: "More" }} context={status && `You have version ${status.current}`} />
  );
  if (!status) {
    return (
      <>
        {header}
        <Card>
          <p>This copy of TourLedger does not update itself.</p>
        </Card>
      </>
    );
  }
  const phase = status.phase;
  return (
    <>
      {header}
      <Card>
        <p role="status" className={status.error ? "error" : undefined}>
          {describe(status)}
        </p>
        {phase === "downloading" && <Progress status={status} />}
        {status.notes && (phase === "available" || phase === "ready") && <p className="muted">{status.notes}</p>}
        <div className="actions">
          {(phase === "ready" || phase === "needs_permission") && (
            <Button variant="primary" icon={RefreshCw} onClick={() => void installUpdate()}>
              {installLabel()}
            </Button>
          )}
          {phase === "available" && (
            <Button variant="primary" icon={Download} onClick={() => void downloadUpdate()}>
              Download
            </Button>
          )}
          <Button disabled={busy(phase)} onClick={() => void checkForUpdate()}>
            Check for updates
          </Button>
        </div>
      </Card>
      <Card>
        <Toggle
          label="Check automatically"
          hint="Asks GitHub when the app opens, and daily while it stays open, and downloads a newer version. Nothing from your ledger is sent; GitHub sees your internet address, as any website does."
          checked={preferences.checkUpdates}
          onChange={(checkUpdates) => update({ checkUpdates })}
        />
      </Card>
    </>
  );
}

/** Asks once for each version, and never over a form or while something is being typed. */
export function UpdatePrompt() {
  const status = useUpdate();
  const [asked, setAsked] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const ready = status?.phase === "ready" ? status.version : null;

  useEffect(() => {
    if (!ready || ready === asked || open) return;
    const timer = setInterval(() => {
      const typing = document.activeElement?.matches("input, textarea, select");
      if (!typing && !document.querySelector("dialog[open]")) setOpen(true);
    }, 1000);
    return () => clearInterval(timer);
  }, [ready, asked, open]);

  if (!open || !ready || !status) return null;
  const later = () => {
    setAsked(ready);
    setOpen(false);
  };
  return (
    <Sheet title="Update ready" onClose={later}>
      <p>{describe(status)}</p>
      {status.notes && <p className="muted">{status.notes}</p>}
      <div className="form-actions">
        <Button onClick={later}>Later</Button>
        <Button
          variant="primary"
          onClick={() => {
            later();
            void installUpdate();
          }}
        >
          {installLabel()}
        </Button>
      </div>
    </Sheet>
  );
}
