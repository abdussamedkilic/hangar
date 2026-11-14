import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type ScanReport } from "../lib/api";
import Modal from "./Modal";
import ScanDialog from "./ScanDialog";
import AccountsSection from "./AccountsSection";

interface Props {
  sidebar: "open" | "collapsed";
  onSidebar?: (v: "open" | "collapsed") => void;
  onResetTheme?: () => Promise<void>;
  onScanned?: (r: ScanReport) => void;
  onAccountsChanged?: (label?: string) => Promise<void>;
  onClose: () => void;
}

export default function SettingsDialog({ sidebar, onSidebar, onResetTheme, onScanned, onAccountsChanged, onClose }: Props) {
  const [tab, setTab] = useState<"accounts" | "custom">("accounts");
  const [error, setError] = useState<string | null>(null);
  const [folders, setFolders] = useState<string[]>([]);
  const [scanning, setScanning] = useState(false);

  useEffect(() => {
    api.getScanFolders().then(setFolders).catch((err) => setError(String(err)));
  }, []);

  const saveFolders = async (next: string[]) => {
    setFolders(next);
    try {
      await api.setScanFolders(next);
    } catch (err) {
      setError(String(err));
    }
  };

  const addFolder = async () => {
    try {
      const f = await open({ directory: true });
      if (typeof f === "string" && !folders.includes(f)) await saveFolders([...folders, f]);
    } catch (err) {
      setError(String(err));
    }
  };

  const pickSidebar = (v: "open" | "collapsed") => {
    onSidebar?.(v);
  };

  const TABS = [{ id: "accounts", label: "Hesaplar" }, { id: "custom", label: "Özelleştir" }] as const;
  const tabKey = (e: KeyboardEvent, i: number) => {
    const next = e.key === "ArrowDown" || e.key === "ArrowRight" ? i + 1 : e.key === "ArrowUp" || e.key === "ArrowLeft" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? TABS.length - 1 : null;
    if (next === null) return;
    e.preventDefault();
    const t = TABS[(next + TABS.length) % TABS.length];
    setTab(t.id);
    document.getElementById(`settings-tab-${t.id}`)?.focus();
  };

  const scanningRef = useRef(false);
  scanningRef.current = scanning;
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const modalClose = useCallback(() => {
    if (!scanningRef.current) onCloseRef.current();
  }, []);

  return (
    <Modal label="Ayarlar" onClose={modalClose}>
      <h2>Ayarlar</h2>
      <div className="settings-body">
        <div className="settings-tabs" role="tablist" aria-orientation="vertical" aria-label="Ayar bölümleri">
          {TABS.map((t, i) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              id={`settings-tab-${t.id}`}
              aria-selected={tab === t.id}
              aria-controls={`settings-panel-${t.id}`}
              tabIndex={tab === t.id ? 0 : -1}
              onClick={() => setTab(t.id)}
              onKeyDown={(e) => tabKey(e, i)}
            >
              {t.label}
            </button>
          ))}
        </div>
        <div className="settings-panel" role="tabpanel" id="settings-panel-accounts" aria-labelledby="settings-tab-accounts" hidden={tab !== "accounts"}>
          <AccountsSection onChanged={onAccountsChanged ?? (async () => {})} />
        </div>
        <div className="settings-panel" role="tabpanel" id="settings-panel-custom" aria-labelledby="settings-tab-custom" hidden={tab !== "custom"}>
          <div className="fld">
            <span>Tema</span>
            <div className="row">
              <button type="button" className="ghost" onClick={() => api.openThemeFile().catch((err) => setError(String(err)))}>Tema dosyasını aç</button>
              <button type="button" className="ghost" onClick={() => onResetTheme?.().catch((err) => setError(String(err)))}>Varsayılana dön</button>
            </div>
          </div>
          <div className="fld">
            <span id="settings-sidebar-label">Açılışta menü</span>
            <div className="seg" role="radiogroup" aria-labelledby="settings-sidebar-label">
              {([["open", "Açık"], ["collapsed", "Daraltılmış"]] as const).map(([v, l]) => (
                <button key={v} type="button" role="radio" aria-checked={sidebar === v} onClick={() => pickSidebar(v)}>{l}</button>
              ))}
            </div>
          </div>
          <div className="fld">
            <span>Oyun klasörleri</span>
            <ul className="scanlist">
              {folders.map((f) => (
                <li key={f}>
                  <code>{f}</code>
                  <button type="button" className="ghost" onClick={() => saveFolders(folders.filter((x) => x !== f))}>Kaldır</button>
                </li>
              ))}
            </ul>
            <div className="row">
              <button type="button" className="ghost" onClick={addFolder}>Klasör ekle</button>
              <button type="button" className="ghost" disabled={folders.length === 0} onClick={() => setScanning(true)}>Klasörleri tara</button>
            </div>
          </div>
        </div>
      </div>
      {error && <p className="err" role="alert">{error}</p>}
      <div className="acts">
        <button type="button" className="primary" onClick={onClose}>Kapat</button>
      </div>
      {scanning && <ScanDialog onDone={(r) => onScanned?.(r)} onClose={() => setScanning(false)} />}
    </Modal>
  );
}
