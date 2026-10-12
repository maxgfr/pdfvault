import type { CompressReport } from "@pdfvault/engine";
import { useEffect, useMemo } from "react";

export const bytes = (n: number) =>
  n < 1024 ? `${n} B` : n < 1024 * 1024 ? `${(n / 1024).toFixed(1)} KB` : `${(n / 1024 / 1024).toFixed(2)} MB`;

function Compress({ r }: { r: CompressReport }) {
  const pct = r.inputBytes ? Math.round((1 - r.outputBytes / r.inputBytes) * 100) : 0;
  const changed = r.images.filter((i) => i.action !== "kept" && i.action !== "skipped");
  return (
    <div className="report compress-report">
      <div className="gain" data-kept={r.keptOriginal}>
        <span className="big">{r.keptOriginal ? "0%" : `−${pct}%`}</span>
        <span>
          {bytes(r.inputBytes)} → <strong>{bytes(r.outputBytes)}</strong>
        </span>
      </div>
      <div className="bar" role="img" aria-label={`Output is ${100 - pct}% of the original size`}>
        <span ref={(el) => el?.style.setProperty("--w", `${Math.max(2, 100 - pct)}%`)} />
      </div>
      {r.keptOriginal ? (
        <p className="note">This file was already as small as this level can make it, so you get the original back. Try a stronger level.</p>
      ) : null}
      <ul className="facts">
        {changed.length ? <li>{changed.length} image{changed.length > 1 ? "s" : ""} re-encoded</li> : null}
        {r.streamsRecompressed ? <li>{r.streamsRecompressed} streams recompressed</li> : null}
        {r.duplicatesMerged ? <li>{r.duplicatesMerged} duplicates merged</li> : null}
        {r.removed.map((x) => (
          <li key={x}>removed {x.replace(":", " ×")}</li>
        ))}
      </ul>
      {r.images.length ? (
        <details>
          <summary>Images ({r.images.length})</summary>
          <table>
            <thead>
              <tr>
                <th>Size</th>
                <th>DPI</th>
                <th>Before</th>
                <th>After</th>
                <th>Action</th>
              </tr>
            </thead>
            <tbody>
              {r.images.map((i) => (
                <tr key={i.object}>
                  <td>
                    {i.width}×{i.height}
                    {i.newWidth !== i.width ? ` → ${i.newWidth}×${i.newHeight}` : ""}
                  </td>
                  <td>{i.dpi ? Math.round(i.dpi) : "–"}</td>
                  <td>{bytes(i.bytesBefore)}</td>
                  <td>{bytes(i.bytesAfter)}</td>
                  <td title={i.reason}>{i.action}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </details>
      ) : null}
    </div>
  );
}

function KeyValues({ data }: { data: Record<string, unknown> }) {
  const rows = Object.entries(data).filter(([, v]) => v !== null && v !== undefined && v !== "");
  return (
    <dl className="kv">
      {rows.map(([k, v]) => (
        <div key={k}>
          <dt>{k.replace(/([A-Z])/g, " $1").toLowerCase()}</dt>
          <dd>{typeof v === "object" ? <pre>{JSON.stringify(v, null, 2)}</pre> : String(v)}</dd>
        </div>
      ))}
    </dl>
  );
}

interface Diff {
  page: number;
  changed: number;
  missingIn: string | null;
}

function Compare({ r, images }: { r: { identical: boolean; pages: Diff[] }; images: { name: string; bytes: Uint8Array }[] }) {
  const urls = useMemo(() => images.map((f) => URL.createObjectURL(new Blob([f.bytes as BlobPart], { type: "image/png" }))), [images]);
  useEffect(() => () => urls.forEach((u) => URL.revokeObjectURL(u)), [urls]);
  return (
    <div className="report">
      <p className="verdict">{r.identical ? "No visual difference." : "Differences found — changed pixels are red."}</p>
      <ul className="facts">
        {r.pages.map((p) => (
          <li key={p.page}>
            Page {p.page + 1}: {p.missingIn ? `only in the ${p.missingIn === "first" ? "second" : "first"} file` : `${(p.changed * 100).toFixed(2)}% changed`}
          </li>
        ))}
      </ul>
      <div className="diffs">
        {urls.map((u, i) => (
          <figure key={u}>
            <img src={u} alt={`Differences on ${images[i]!.name}`} />
            <figcaption>{images[i]!.name}</figcaption>
          </figure>
        ))}
      </div>
    </div>
  );
}

export function Report({ toolId, report, files }: { toolId: string; report: unknown; files: { name: string; bytes: Uint8Array }[] }) {
  if (report === null || report === undefined) return null;
  if (toolId === "compress") return <Compress r={report as CompressReport} />;
  if (toolId === "compare") return <Compare r={report as { identical: boolean; pages: Diff[] }} images={files.filter((f) => f.name.endsWith(".png"))} />;
  if (toolId === "info") {
    const r = report as Record<string, unknown> & { metadata: Record<string, string>; security: Record<string, unknown> };
    const { metadata, security, pageSizes, fonts, ...rest } = r;
    return (
      <div className="report">
        <KeyValues data={rest} />
        <h4>Protection</h4>
        <KeyValues data={security} />
        {Object.keys(metadata ?? {}).length ? (
          <>
            <h4>Metadata</h4>
            <KeyValues data={metadata} />
          </>
        ) : null}
        <details>
          <summary>Pages and fonts</summary>
          <pre>{JSON.stringify({ pageSizes, fonts }, null, 2)}</pre>
        </details>
      </div>
    );
  }
  if (toolId === "meta" && typeof report === "object" && report && "info" in report) {
    const r = report as { info: Record<string, string>; hasXmp: boolean };
    return (
      <div className="report">
        {Object.keys(r.info).length ? <KeyValues data={r.info} /> : <p>No document information.</p>}
        <p className="note">{r.hasXmp ? "An XMP metadata packet is also present." : "No XMP packet."}</p>
      </div>
    );
  }
  if (typeof report === "object") {
    return (
      <div className="report">
        <KeyValues data={report as Record<string, unknown>} />
      </div>
    );
  }
  return null;
}
