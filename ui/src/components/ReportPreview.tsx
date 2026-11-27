// The report's pages exactly as the PDF will have them: Rust lays them out once,
// and this draws the same marks.
import { ZoomIn, ZoomOut } from "lucide-react";
import { useEffect, useState } from "react";
import { api, today } from "../api";
import { jpegUrl } from "../pictures";
import { useApp } from "../state";
import type { Preview, PreviewMark } from "../types";
import { Button } from "./ui";

const grey = (g: number) => `rgb(${g}, ${g}, ${g})`;

function Mark({ mark, images }: { mark: PreviewMark; images: Preview["images"] }) {
  if (mark.kind === "box") {
    return <rect x={mark.x} y={mark.y} width={mark.w} height={mark.h} fill={grey(mark.grey)} />;
  }
  if (mark.kind === "image") {
    const data = images[mark.hash];
    return data ? (
      <image href={jpegUrl(data)} x={mark.x} y={mark.y} width={mark.w} height={mark.h} preserveAspectRatio="none" />
    ) : null;
  }
  return (
    // Held to the PDF's width, so a different screen font cannot shift layout.
    <text
      x={mark.x}
      y={mark.y}
      fontSize={mark.size}
      fontWeight={mark.bold ? 700 : 400}
      fill={grey(mark.grey)}
      textLength={mark.width}
      lengthAdjust="spacingAndGlyphs"
    >
      {mark.text}
    </text>
  );
}

export function ReportPreview({ tourId, detail }: { tourId: string; detail: boolean }) {
  const { data, fail } = useApp();
  const [preview, setPreview] = useState<Preview | null>(null);
  const [large, setLarge] = useState(false);

  useEffect(() => {
    let stale = false;
    api<Preview>({ op: "report_preview", tour_id: tourId, today: today(), detail })
      .then((p) => !stale && setPreview(p))
      .catch((e) => !stale && fail(e));
    return () => {
      stale = true;
    };
  }, [tourId, detail, data.revision, fail]);

  if (!preview) return <p className="muted">Preparing the preview…</p>;
  const total = preview.pages.length;

  return (
    <>
      <div className="actions">
        <span className="muted">
          {total} {total === 1 ? "page" : "pages"}
        </span>
        <Button icon={large ? ZoomOut : ZoomIn} onClick={() => setLarge(!large)}>
          {large ? "Fit to screen" : "Larger"}
        </Button>
      </div>
      <div className={large ? "pages large" : "pages"}>
        {preview.pages.map((marks, i) => (
          <svg
            key={i}
            className="page"
            viewBox={`0 0 ${preview.width} ${preview.height}`}
            role="img"
            aria-label={`Page ${i + 1} of ${total}`}
          >
            <rect width={preview.width} height={preview.height} fill="#fff" />
            {marks.map((mark, n) => (
              <Mark key={n} mark={mark} images={preview.images} />
            ))}
          </svg>
        ))}
      </div>
    </>
  );
}
