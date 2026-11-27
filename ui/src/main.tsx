import { ChartColumn, House, Landmark, Luggage, Menu } from "lucide-react";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { Overlays } from "./components/ui";
import { listenForBack } from "./overlays";
import { Home } from "./pages/Home";
import { ClaimReport } from "./pages/ClaimReport";
import { LoanDashboard } from "./pages/loans/Dashboard";
import { LoanDetail } from "./pages/loans/Detail";
import { LoanInsights } from "./pages/loans/Insights";
import { LoanList } from "./pages/loans/List";
import { Tracker } from "./pages/loans/Tracker";
import { Reports } from "./pages/Reports";
import { Appearance, Companies, CompanyPage, DataPage, More, People, ProfilePage, QuickEntries } from "./pages/More";
import { Sync } from "./pages/Sync";
import { TourDetail } from "./pages/TourDetail";
import { Tours } from "./pages/Tours";
import { UpdatePrompt, Updates } from "./pages/Updates";
import { applyPreferences } from "./preferences";
import { useRoute } from "./router";
import { AppProvider } from "./state";
import { startUpdates } from "./updates";
import "./styles/theme.css";
import "./styles/app.css";

const NAV = [
  { path: "home", label: "Home", icon: House },
  { path: "tours", label: "Tours", icon: Luggage },
  { path: "reports", label: "Reports", icon: ChartColumn },
  { path: "loans", label: "Loans", icon: Landmark },
  { path: "more", label: "More", icon: Menu },
];

function Page({ section, id, extra }: { section: string; id?: string; extra?: string }) {
  switch (section) {
    case "tours":
      if (id && extra === "report") return <ClaimReport key={id} id={id} />;
      return id ? <TourDetail key={id} id={id} /> : <Tours />;
    case "reports":
      return <Reports />;
    case "loans":
      if (id === "list") return <LoanList />;
      if (id === "tracker") return <Tracker />;
      if (id === "insights") return <LoanInsights />;
      if (id === "l" && extra) return <LoanDetail key={extra} id={extra} />;
      return <LoanDashboard />;
    case "more":
      switch (id) {
        case "profile":
          return <ProfilePage />;
        case "people":
          return <People />;
        case "companies":
          return extra ? <CompanyPage key={extra} id={extra} /> : <Companies />;
        case "quick":
          return <QuickEntries />;
        case "sync":
          return <Sync />;
        case "data":
          return <DataPage />;
        case "appearance":
          return <Appearance />;
        case "updates":
          return <Updates />;
        default:
          return <More />;
      }
    default:
      return <Home />;
  }
}

/** Which way a page comes in: deeper from the right, back from the left. */
function motion(from: string, to: string): "up" | "forward" | "back" {
  const [a, b] = [from.split("/"), to.split("/")];
  if (a[0] !== b[0] || a.length === b.length) return "up";
  return b.length > a.length ? "forward" : "back";
}

function Shell() {
  const [section = "home", id, extra] = useRoute();
  const place = [section, id, extra].filter(Boolean).join("/");
  const [shown, setShown] = useState({ place, motion: "up" });
  // Worked out while rendering, so the new page starts with the right motion.
  if (shown.place !== place) setShown({ place, motion: motion(shown.place, place) });
  useEffect(() => {
    window.scrollTo(0, 0);
  }, [place]);
  return (
    <div className="shell">
      <nav className="nav glass" aria-label="Main">
        <span className="brand">TourLedger</span>
        {NAV.map(({ path, label, icon: Icon }) => (
          <a key={path} href={`#/${path}`} aria-current={section === path ? "page" : undefined}>
            <Icon size={21} aria-hidden />
            {label}
          </a>
        ))}
      </nav>
      {/* An attribute, not a class: "back" is also the style of the back link. */}
      <main key={shown.place} data-motion={shown.motion}>
        <Page section={section} id={id} extra={extra} />
      </main>
      <Overlays />
      <UpdatePrompt />
    </div>
  );
}

applyPreferences();
listenForBack();
startUpdates();
createRoot(document.getElementById("root")!).render(
  <AppProvider>
    <Shell />
  </AppProvider>,
);
