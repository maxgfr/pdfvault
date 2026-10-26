import { useEffect, useRef, useState } from "react";
import { Icon } from "../icons.tsx";

export const hasCamera = () => typeof navigator !== "undefined" && Boolean(navigator.mediaDevices?.getUserMedia);

type State = { kind: "starting" } | { kind: "live"; torch: boolean | null } | { kind: "failed"; message: string };

function failure(e: unknown): string {
  const name = e instanceof DOMException ? e.name : "";
  if (name === "NotAllowedError" || name === "SecurityError") return "Camera access was refused. Allow it in the browser's site settings, or take the photo with your camera app.";
  if (name === "NotFoundError" || name === "OverconstrainedError") return "No camera was found on this device.";
  if (name === "NotReadableError") return "The camera is busy in another app. Close it and try again.";
  return "The camera could not start.";
}

/**
 * Full-screen viewfinder. Each press of the shutter hands a JPEG of the
 * current frame to `onShot`; nothing is recorded or sent anywhere.
 */
export function Camera({
  count,
  last,
  onShot,
  onPick,
  onDone,
}: {
  count: number;
  /** Object URL of the latest page, shown as a reminder next to the shutter. */
  last?: string;
  onShot: (jpeg: Uint8Array) => void;
  /** Photos chosen with the system picker or camera app instead. */
  onPick: (files: File[]) => void;
  onDone: () => void;
}) {
  const video = useRef<HTMLVideoElement>(null);
  const stream = useRef<MediaStream | null>(null);
  const picker = useRef<HTMLInputElement>(null);
  const system = useRef<HTMLInputElement>(null);
  const [state, setState] = useState<State>({ kind: "starting" });
  const [torchOn, setTorchOn] = useState(false);
  const [flash, setFlash] = useState(0);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let stopped = false;
    const stop = () => {
      for (const t of stream.current?.getTracks() ?? []) t.stop();
      stream.current = null;
    };
    const start = async () => {
      if (!hasCamera()) {
        setState({ kind: "failed", message: "This browser cannot use the camera here. Take the photo with your camera app instead." });
        return;
      }
      setState({ kind: "starting" });
      try {
        const s = await navigator.mediaDevices.getUserMedia({
          audio: false,
          video: { facingMode: { ideal: "environment" }, width: { ideal: 3840 }, height: { ideal: 2160 } },
        });
        if (stopped) {
          for (const t of s.getTracks()) t.stop();
          return;
        }
        stream.current = s;
        const v = video.current;
        if (v) {
          v.srcObject = s;
          await v.play().catch(() => {});
        }
        const track = s.getVideoTracks()[0];
        const caps = (track?.getCapabilities?.() ?? {}) as MediaTrackCapabilities & { torch?: boolean };
        setState({ kind: "live", torch: caps.torch ? false : null });
      } catch (e) {
        if (!stopped) setState({ kind: "failed", message: failure(e) });
      }
    };
    // The camera is released whenever the app is hidden, and taken back on return.
    const visibility = () => {
      if (document.hidden) {
        stop();
        setTorchOn(false);
      } else if (!stream.current) void start();
    };
    void start();
    document.addEventListener("visibilitychange", visibility);
    return () => {
      stopped = true;
      document.removeEventListener("visibilitychange", visibility);
      stop();
    };
  }, []);

  const shoot = async () => {
    const v = video.current;
    if (!v || !v.videoWidth || busy) return;
    setBusy(true);
    setFlash((n) => n + 1);
    try {
      const canvas = document.createElement("canvas");
      canvas.width = v.videoWidth;
      canvas.height = v.videoHeight;
      canvas.getContext("2d")!.drawImage(v, 0, 0);
      const blob = await new Promise<Blob | null>((done) => canvas.toBlob(done, "image/jpeg", 0.92));
      if (blob) onShot(new Uint8Array(await blob.arrayBuffer()));
    } finally {
      setBusy(false);
    }
  };

  const toggleTorch = async () => {
    const track = stream.current?.getVideoTracks()[0];
    if (!track) return;
    const next = !torchOn;
    try {
      await track.applyConstraints({ advanced: [{ torch: next } as MediaTrackConstraintSet] });
      setTorchOn(next);
    } catch {
      setState((s) => (s.kind === "live" ? { ...s, torch: null } : s));
    }
  };

  return (
    <div className="cam" role="dialog" aria-modal="true" aria-label="Camera">
      <div className="cam-top">
        <button type="button" className="cam-round" onClick={onDone} aria-label={count ? "Done, see the pages" : "Close the camera"}>
          <Icon name={count ? "back" : "x"} />
        </button>
        <p className="cam-tip">{state.kind === "live" ? "Fit the whole sheet in the frame, on a darker surface." : null}</p>
        {state.kind === "live" && state.torch !== null ? (
          <button type="button" className="cam-round" aria-pressed={torchOn} onClick={() => void toggleTorch()} aria-label="Flashlight">
            <Icon name="flash" />
          </button>
        ) : (
          <span className="cam-round-spacer" />
        )}
      </div>

      <div className="cam-view">
        <video ref={video} playsInline muted autoPlay aria-label="Camera preview" />
        {flash ? <span key={flash} className="cam-flash" aria-hidden="true" /> : null}
        {state.kind === "starting" ? <p className="cam-msg">Starting the camera…</p> : null}
        {state.kind === "failed" ? (
          <div className="cam-msg">
            <p>{state.message}</p>
            <button type="button" className="primary" onClick={() => system.current?.click()}>
              <Icon name="camera" /> Take a photo
            </button>
            {/* The camera app, when the browser cannot use the camera itself. */}
            <input
              ref={system}
              type="file"
              accept="image/*"
              capture="environment"
              hidden
              aria-label="Photo from the camera app"
              onChange={(e) => {
                onPick([...(e.currentTarget.files ?? [])]);
                e.currentTarget.value = "";
              }}
            />
          </div>
        ) : null}
      </div>

      <div className="cam-bottom">
        <button type="button" className="cam-thumb" onClick={onDone} disabled={!count} aria-label={`${count} page${count === 1 ? "" : "s"}, review them`}>
          {last ? <img src={last} alt="" /> : null}
          {count ? <span className="cam-count">{count}</span> : null}
        </button>
        <button
          type="button"
          className="cam-shutter"
          onClick={() => void shoot()}
          disabled={state.kind !== "live" || busy}
          aria-label="Take the picture"
        />
        <button type="button" className="cam-round" onClick={() => picker.current?.click()} aria-label="Choose photos">
          <Icon name="image" />
        </button>
        <input
          ref={picker}
          type="file"
          accept="image/*"
          multiple
          hidden
          aria-label="Photos to scan"
          onChange={(e) => {
            onPick([...(e.currentTarget.files ?? [])]);
            e.currentTarget.value = "";
          }}
        />
      </div>
    </div>
  );
}
