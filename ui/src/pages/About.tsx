// More → About: what the app is and who makes it. The links are the only ones the
// app may open; each is also allowed in src-tauri/capabilities.
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { BriefcaseBusiness, Code, ExternalLink, FileText, Globe, ScrollText, type LucideIcon } from "lucide-react";
import { version } from "../../package.json";
import { Card, List, ListRow, PageHeader } from "../components/ui";
import { useApp } from "../state";

const REPOSITORY = "https://github.com/sudsarkar13/tourledger";

interface Link {
  title: string;
  detail: string;
  url: string;
  icon: LucideIcon;
}

const PROJECT: Link[] = [
  { title: "Source code", detail: "github.com/sudsarkar13/tourledger", url: REPOSITORY, icon: Code },
  { title: "What changed in each version", detail: "Release notes on GitHub", url: `${REPOSITORY}/releases`, icon: ScrollText },
  { title: "Licence", detail: "MIT: free to use, change and share", url: `${REPOSITORY}/blob/main/LICENSE`, icon: FileText },
];

const DEVELOPER: Link[] = [
  { title: "Website", detail: "sudeeptasarkar.in", url: "https://www.sudeeptasarkar.in/", icon: Globe },
  { title: "GitHub", detail: "github.com/sudsarkar13", url: "https://github.com/sudsarkar13", icon: Code },
  {
    title: "LinkedIn",
    detail: "linkedin.com/in/sudeepta-sarkar",
    url: "https://www.linkedin.com/in/sudeepta-sarkar",
    icon: BriefcaseBusiness,
  },
];

function Links({ links }: { links: Link[] }) {
  const { fail } = useApp();
  const open = (url: string) => {
    if (isTauri()) openUrl(url).catch(fail);
    else window.open(url, "_blank", "noopener,noreferrer");
  };
  return (
    <List>
      {links.map((l) => (
        <ListRow
          key={l.url}
          icon={l.icon}
          title={l.title}
          detail={l.detail}
          note={<ExternalLink size={16} aria-label="Opens in your browser" />}
          onClick={() => open(l.url)}
        />
      ))}
    </List>
  );
}

export function About() {
  return (
    <>
      <PageHeader title="About" back={{ href: "#/more", label: "More" }} context={`TourLedger ${version}`} />
      <Card title="TourLedger">
        <p>
          Keeps what you spend on tours and events, what your company gives and still owes, and your loans and instalments. It
          makes the claim you hand in.
        </p>
        <p className="muted">
          No account and no server of its own. Your ledger stays on your devices, which keep each other up to date when you
          link them.
        </p>
        <Links links={PROJECT} />
      </Card>
      <Card title="Developer">
        <p>
          <strong>Sudeepta Sarkar</strong>
          <br />
          <span className="muted">Software developer</span>
        </p>
        <p>
          Software Developer at Lex Protector LLP, and formerly Chief Technology Officer at Healthunity Solutions (Dr. Reach).
          Builds web applications with React, Next.js and Tailwind CSS.
        </p>
        <p>
          B.Tech in Computer Science and Engineering from KIIT University, Bhubaneswar (2024), after a diploma in the same from
          KIIT Polytechnic.
        </p>
        <Links links={DEVELOPER} />
      </Card>
      <p className="muted">© 2026 Sudeepta Sarkar</p>
    </>
  );
}
