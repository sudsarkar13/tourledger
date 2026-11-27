// More → Notifications: reminders for loan instalments on this device.
import { Bell } from "lucide-react";
import { useEffect, useState } from "react";
import { isMobile } from "../api";
import { Button, Card, EmptyState, List, ListRow, PageHeader, Select, Toggle } from "../components/ui";
import { day } from "../format";
import { getNotifier, refreshReminders, reminderList } from "../notifications";
import { usePreferences } from "../preferences";
import { momentOf, type Reminder } from "../reminderPlan";
import { useApp } from "../state";

const DAYS = [
  { value: "0", label: "On the day only" },
  { value: "1", label: "1 day before" },
  { value: "3", label: "3 days before" },
  { value: "7", label: "7 days before" },
];
const HOURS = [
  { value: "8", label: "8 in the morning" },
  { value: "9", label: "9 in the morning" },
  { value: "12", label: "12 noon" },
  { value: "18", label: "6 in the evening" },
  { value: "21", label: "9 at night" },
];

/** Keeps this device's reminders matching the ledger and the settings. */
export function Reminders() {
  const { data } = useApp();
  const { preferences } = usePreferences();
  const { reminders, remindDays, remindHour, remindMissed, remindAmounts } = preferences;
  useEffect(() => {
    const run = () => void refreshReminders(preferences);
    run();
    // A computer shows them itself, so it looks again as the day goes on.
    const timer = setInterval(run, 15 * 60 * 1000);
    document.addEventListener("visibilitychange", run);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", run);
    };
  }, [data.revision, reminders, remindDays, remindHour, remindMissed, remindAmounts]);
  return null;
}

export function Notifications() {
  const { data, notify } = useApp();
  const { preferences, update } = usePreferences();
  const [refused, setRefused] = useState(false);
  const [coming, setComing] = useState<Reminder[]>([]);
  const notifier = getNotifier();
  const header = <PageHeader title="Notifications" back={{ href: "#/more", label: "More" }} context="For this device" />;

  useEffect(() => {
    let stale = false;
    reminderList(preferences)
      .then((list) => {
        const now = new Date();
        if (!stale) setComing(list.filter((r) => momentOf(r.on, preferences.remindHour) > now).slice(0, 5));
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [data.revision, preferences.remindDays, preferences.remindHour, preferences.remindMissed]);

  if (!notifier) {
    return (
      <>
        {header}
        <Card>
          <p>Notifications work in the installed app.</p>
        </Card>
      </>
    );
  }

  const turn = async (on: boolean) => {
    setRefused(false);
    if (on && !(await notifier.ask())) {
      setRefused(true);
      return;
    }
    update({ reminders: on });
  };

  const test = async () => {
    if (!(await notifier.ask())) {
      setRefused(true);
      return;
    }
    await notifier.send({ id: 1, title: "TourLedger", body: "Notifications work on this device." });
    notify("Test notification sent");
    void refreshReminders(preferences);
  };

  return (
    <>
      {header}
      <Card>
        <Toggle
          label="Remind me about EMIs"
          hint={
            isMobile
              ? "Arrives with TourLedger closed, sometimes late on battery saver. Open it now and then so changes from your other devices count."
              : "On a computer, reminders arrive while TourLedger is open."
          }
          checked={preferences.reminders}
          onChange={(on) => void turn(on)}
        />
        {refused && (
          <p className="error" role="alert">
            Notifications are not allowed. Allow them for TourLedger in this device's settings, then try again.
          </p>
        )}
        {preferences.reminders && (
          <>
            <div className="grid-2">
              <Select
                label="Remind me"
                value={String(preferences.remindDays)}
                onChange={(x) => update({ remindDays: Number(x) })}
                options={DAYS}
              />
              <Select
                label="At"
                value={String(preferences.remindHour)}
                onChange={(x) => update({ remindHour: Number(x) })}
                options={HOURS}
              />
            </div>
            <Toggle
              label="Tell me if one is not marked paid"
              hint="Once, the day after it was due"
              checked={preferences.remindMissed}
              onChange={(remindMissed) => update({ remindMissed })}
            />
            <Toggle
              label="Show amounts"
              hint="Off by default: notifications can be read on a locked screen."
              checked={preferences.remindAmounts}
              onChange={(remindAmounts) => update({ remindAmounts })}
            />
          </>
        )}
        <div className="actions">
          <Button icon={Bell} onClick={() => void test()}>
            Send a test notification
          </Button>
        </div>
      </Card>
      {preferences.reminders && (
        <Card title="Coming up">
          {coming.length ? (
            <List>
              {coming.map((r) => (
                <ListRow
                  key={r.key}
                  // Named by due date: "due today" reads wrongly in a list of what is to come.
                  title={`${r.name}: due ${day(r.due)}`}
                  detail={`Reminder on ${day(r.on)}${r.kind === "missed" ? ", only if not marked paid" : ""}`}
                  href={`#/loans/l/${r.loan_id}`}
                />
              ))}
            </List>
          ) : (
            <EmptyState icon={Bell} title="Nothing to remind you of in the next two months." />
          )}
        </Card>
      )}
    </>
  );
}
