import { MODEL_PACKS, megabytes, packBytes, type ModelPackId } from "@pdfvault/engine";
import { useState } from "react";
import { Icon } from "./icons.tsx";
import { downloadPack } from "./models.ts";

interface Props {
  pack: ModelPackId;
  /** Called once the model is on the device; the caller runs the job again. */
  onReady: () => void;
  onCancel: () => void;
}

/** Asks before downloading an OCR model, then downloads it. */
export function ModelPrompt({ pack, onReady, onCancel }: Props) {
  const [fraction, setFraction] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const model = MODEL_PACKS[pack];
  const start = async () => {
    setError(null);
    setFraction(0);
    try {
      await downloadPack(pack, setFraction);
      onReady();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setFraction(null);
    }
  };
  return (
    <div className="locked model-prompt" role="alert">
      <Icon name="download" />
      <div>
        <p>
          Text recognition needs <strong>{model.title}</strong>: a one-time {megabytes(packBytes(pack))} download from this
          site, kept on this device so OCR also works offline. Your files are not sent anywhere. Delete it any time from{" "}
          <a href="#/privacy">Privacy</a>.
        </p>
        {fraction !== null ? (
          <div className="progress" role="status" aria-live="polite">
            <div className="track">
              <span ref={(el) => el?.style.setProperty("--p", `${Math.round(fraction * 100)}%`)} />
            </div>
            <p>Downloading… {Math.round(fraction * 100)}%</p>
          </div>
        ) : null}
        {error ? <p className="error-text">{error}</p> : null}
        <div className="actions">
          <button type="button" className="primary" disabled={fraction !== null} onClick={() => void start()}>
            <Icon name="download" /> Download and continue
          </button>
          <button type="button" className="secondary" disabled={fraction !== null} onClick={onCancel}>
            Not now
          </button>
        </div>
      </div>
    </div>
  );
}
