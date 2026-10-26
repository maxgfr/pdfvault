// Inline stroke icons (24×24, 1.75 stroke): no icon font, no external requests.
const PATHS: Record<string, string> = {
  file: "M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8zM14 3v5h5",
  image: "M4 5h16v14H4zM4 15l4-4 4 4 3-3 5 5M15.5 9.5h.01",
  lock: "M6 11h12v9H6zM8.5 11V8a3.5 3.5 0 0 1 7 0v3",
  unlock: "M6 11h12v9H6zM8.5 11V8a3.5 3.5 0 0 1 6.8-1.2",
  shield: "M12 3l7 3v5c0 4.5-3 8.2-7 10-4-1.8-7-5.5-7-10V6z",
  offline: "M3 3l18 18M8.5 16.5a5 5 0 0 1 7 0M5 13a10 10 0 0 1 5-2.7M12 20h.01M19 13a10 10 0 0 0-2.2-1.6",
  download: "M12 4v11M7 10l5 5 5-5M5 20h14",
  back: "M15 18l-6-6 6-6",
  forward: "M9 18l6-6-6-6",
  up: "M6 15l6-6 6 6",
  down: "M6 9l6 6 6-6",
  x: "M6 6l12 12M18 6L6 18",
  search: "M11 18a7 7 0 1 1 0-14 7 7 0 0 1 0 14zM20 20l-4-4",
  compress: "M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5",
  organize: "M4 5h7v6H4zM13 5h7v6h-7zM4 13h7v6H4zM13 13h7v6h-7z",
  edit: "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4",
  security: "M12 3l7 3v5c0 4.5-3 8.2-7 10-4-1.8-7-5.5-7-10V6zM9 12l2 2 4-4",
  convert: "M4 8h13l-3-3M20 16H7l3 3",
  terminal: "M4 5h16v14H4zM7 9l3 3-3 3M12 15h5",
  code: "M8 8l-4 4 4 4M16 8l4 4-4 4M14 5l-4 14",
  trash: "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
  view: "M2.5 12S6 5 12 5s9.5 7 9.5 7-3.5 7-9.5 7-9.5-7-9.5-7zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
  minus: "M5 12h14",
  plus: "M12 5v14M5 12h14",
  upload: "M12 16V4M7 9l5-5 5 5M5 20h14",
  cursor: "M5 3l14 7-6 2-2 6z",
  text: "M5 6V4h14v2M12 4v16M9 20h6",
  edittext: "M4 7V5h10v2M9 5v12M7 17h4M13 20v-2.5l6-6 2.5 2.5-6 6z",
  square: "M4 5h16v14H4z",
  whiteout: "M4 5h16v14H4zM8 9h8M8 13h5",
  highlight: "M9 15l-4 4h6l2-2M8 13l7-7 3 3-7 7zM4 21h16",
  pen: "M3 17c3-4 5-1 7-4s3-8 6-8c2 0 3 2 2 4s-6 9-9 12M3 21h18",
  note: "M5 4h14v11l-5 5H5zM14 20v-5h5M8 9h8M8 12h5",
  signature: "M3 17c2-1 3-7 5-7s1 6 3 6 2-4 4-4 1 3 3 3 2-1 3-2M3 21h18",
  redact: "M4 6h16v5H4zM4 15h10v3H4z",
  undo: "M9 14L4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3",
  redo: "M15 14l5-5-5-5M20 9H10a6 6 0 0 0 0 12h3",
  "rotate-cw": "M20 11a8 8 0 1 1-2.3-5.7M20 4v5h-5",
  "rotate-ccw": "M4 11a8 8 0 1 0 2.3-5.7M4 4v5h5",
  copy: "M8 8h12v12H8zM16 8V4H4v12h4",
  page: "M6 3h12v18H6z",
  camera: "M3 8h4l2-3h6l2 3h4v11H3zM12 16.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7z",
  scan: "M4 8V4h4M16 4h4v4M20 16v4h-4M8 20H4v-4M7 12h10",
  share: "M12 3v12M8 7l4-4 4 4M5 12v8h14v-8",
  flash: "M13 3L5 13h6l-1 8 8-10h-6z",
};

export function Icon({ name, label }: { name: string; label?: string }) {
  return (
    <svg
      className="ico"
      viewBox="0 0 24 24"
      width="20"
      height="20"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden={label ? undefined : true}
      role={label ? "img" : undefined}
      aria-label={label}
    >
      <path d={PATHS[name] ?? PATHS.file} />
    </svg>
  );
}
