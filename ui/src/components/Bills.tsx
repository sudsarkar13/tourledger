// Pictures of bills on one expense: choose or photograph, view, remove.
import { Camera, ImagePlus, Trash2 } from "lucide-react";
import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { api, isMobile } from "../api";
import { jpegUrl, toJpeg, type Picture } from "../pictures";
import { useApp } from "../state";
import type { Attachment } from "../types";
import { Button, IconButton, Sheet, Toggle } from "./ui";

export const MAX_BILLS = 10;

function useStoredPicture(attachment: Attachment) {
  const [url, setUrl] = useState("");
  useEffect(() => {
    if (!attachment.present) return;
    let stale = false;
    api<{ data: string }>({ op: "get_attachment", id: attachment.id })
      .then((r) => !stale && setUrl(jpegUrl(r.data)))
      .catch(() => !stale && setUrl(""));
    return () => {
      stale = true;
    };
  }, [attachment.id, attachment.present]);
  return url;
}

function Stored({ bill, number }: { bill: Attachment; number: number }) {
  const { save, remove, fail } = useApp();
  const url = useStoredPicture(bill);
  const [open, setOpen] = useState(false);
  const { id, present: _present, ...body } = bill;
  return (
    <li className="bill">
      <button type="button" className="thumb" aria-label={`View picture ${number}`} onClick={() => setOpen(true)}>
        {url ? <img src={url} alt="" /> : <span className="muted">{bill.present ? "Loading…" : "Not synced yet"}</span>}
      </button>
      <IconButton
        label={`Remove picture ${number}`}
        icon={Trash2}
        onClick={() => void remove("attachment", id, "Remove this picture?", "It is removed from linked devices too.")}
      />
      {open && (
        <Sheet title={`Picture ${number}`} onClose={() => setOpen(false)}>
          {url ? <img className="bill-full" src={url} alt={`Bill picture ${number}`} /> : <p>Not on this device yet.</p>}
          <Toggle
            label="Give it a whole page in the report"
            hint="Otherwise four small bills share a page"
            checked={bill.full_page}
            onChange={(full_page) => void save("attachment", { ...body, full_page }, id).catch(fail)}
          />
        </Sheet>
      )}
    </li>
  );
}

export function BillsField({
  expenseId,
  pending,
  onPending,
}: {
  expenseId?: string;
  /** Chosen but not yet saved: they are stored together with the expense. */
  pending: Picture[];
  onPending: (pictures: Picture[]) => void;
}) {
  const { data, fail } = useApp();
  const stored = expenseId ? data.attachments.filter((a) => a.expense_id === expenseId) : [];
  const files = useRef<HTMLInputElement>(null);
  const camera = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const room = MAX_BILLS - stored.length - pending.length;

  const chosen = async (event: ChangeEvent<HTMLInputElement>) => {
    const input = event.target;
    const list = Array.from(input.files ?? []);
    input.value = "";
    if (!list.length) return;
    setBusy(true);
    try {
      const added: Picture[] = [];
      for (const file of list.slice(0, room)) added.push(await toJpeg(file));
      onPending([...pending, ...added]);
      if (list.length > room) fail(new Error(`Only ${MAX_BILLS} pictures fit on one expense.`));
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <fieldset className="bills">
      <legend>Bills and screenshots</legend>
      {(stored.length > 0 || pending.length > 0) && (
        <ul>
          {stored.map((bill, i) => (
            <Stored key={bill.id} bill={bill} number={i + 1} />
          ))}
          {pending.map((picture, i) => (
            <li className="bill" key={i}>
              <span className="thumb">
                <img src={picture.url} alt={`Picture ${stored.length + i + 1}, not saved yet`} />
              </span>
              <IconButton
                label={`Remove picture ${stored.length + i + 1}`}
                icon={Trash2}
                onClick={() => onPending(pending.filter((_, n) => n !== i))}
              />
            </li>
          ))}
        </ul>
      )}
      <div className="actions">
        {isMobile && (
          <Button icon={Camera} disabled={busy || room <= 0} onClick={() => camera.current?.click()}>
            Take photo
          </Button>
        )}
        <Button icon={ImagePlus} disabled={busy || room <= 0} onClick={() => files.current?.click()}>
          Add picture
        </Button>
      </div>
      <input ref={files} type="file" accept="image/*" multiple hidden onChange={chosen} />
      <input ref={camera} type="file" accept="image/*" capture="environment" hidden onChange={chosen} />
      {pending.length > 0 && <small className="muted">Saved when you save the expense.</small>}
    </fieldset>
  );
}
