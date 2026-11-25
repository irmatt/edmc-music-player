import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface TitleBarProps {
  onSettings: () => void;
  onColumns: () => void;
  onShowHidden: () => void;
  onAbout: () => void;
  onToggle: () => void;
  onNext: () => void;
  onPrevious: () => void;
}

const menus = ["View", "Play", "Help"] as const;
type Menu = (typeof menus)[number];

export function TitleBar({
  onSettings,
  onColumns,
  onShowHidden,
  onAbout,
  onToggle,
  onNext,
  onPrevious,
}: TitleBarProps) {
  const [open, setOpen] = useState<Menu | null>(null);
  const [maximized, setMaximized] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getCurrentWindow().isMaximized().then(setMaximized).catch(() => {});
    const close = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(null);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, []);

  const choose = (action: () => void) => {
    action();
    setOpen(null);
  };

  return (
    <div className="titlebar" ref={root}>
      <div className="titlebar-brand" data-tauri-drag-region>
        <span className="titlebar-icon">♫</span>
        <span>EDMC Music Player</span>
      </div>
      <div className="menubar" data-tauri-drag-region>
        {menus.map((menu) => (
          <div className="menu-wrap" key={menu}>
            <button
              className={open === menu ? "menu-button active" : "menu-button"}
              onClick={() => setOpen(open === menu ? null : menu)}
            >
              {menu}
            </button>
            {open === menu && (
              <div className="menu-popup">
                {menu === "View" && (
                  <>
                    <button onClick={() => choose(onColumns)}>Choose columns…</button>
                    <button onClick={() => choose(onShowHidden)}>Show / hide hidden tracks</button>
                    <button onClick={() => choose(onSettings)}>Settings…</button>
                  </>
                )}
                {menu === "Play" && (
                  <>
                    <button onClick={() => choose(onToggle)}>Play / Pause</button>
                    <button onClick={() => choose(onPrevious)}>Previous</button>
                    <button onClick={() => choose(onNext)}>Next</button>
                  </>
                )}
                {menu === "Help" && (
                  <button onClick={() => choose(onAbout)}>About EDMC Music Player</button>
                )}
              </div>
            )}
          </div>
        ))}
      </div>
      <div className="titlebar-drag-region" data-tauri-drag-region />
      <div className="window-controls">
        <button
          aria-label="Minimise"
          onClick={() => getCurrentWindow().minimize().catch(() => {})}
        >
          —
        </button>
        <button
          aria-label="Maximise"
          onClick={() =>
            (maximized
              ? getCurrentWindow().unmaximize()
              : getCurrentWindow().maximize()
            )
              .then(() => setMaximized(!maximized))
              .catch(() => {})
          }
        >
          {maximized ? "❐" : "□"}
        </button>
        <button
          className="close"
          aria-label="Close"
          onClick={() => getCurrentWindow().close().catch(() => {})}
        >
          ✕
        </button>
      </div>
    </div>
  );
}
