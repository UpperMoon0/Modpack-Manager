import { useEffect, useMemo, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { tfgApi } from "./api";
import { APP_UPDATE_INTERVAL_MS, ROOT_KEY } from "./channel";
import { shouldNotifyAppUpdate } from "./appUpdate";
import type {
  ApplyResult,
  PatchProgress,
  PatchState,
  TfgPlanResponse
} from "./types";

const TFG_UPDATE_INTERVAL_MS = 30 * 60 * 1000;

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function targetLabel(targets: Array<"client" | "server">) {
  return targets.includes("server") ? "Client + server" : "Client only";
}

export default function App() {
  const savedRoot = localStorage.getItem(ROOT_KEY) ?? "";
  const [folderDraft, setFolderDraft] = useState(savedRoot);
  const [root, setRoot] = useState(savedRoot);
  const [tfg, setTfg] = useState<TfgPlanResponse | null>(null);
  const [state, setState] = useState<PatchState | null>(null);
  const [result, setResult] = useState<ApplyResult | null>(null);
  const [progress, setProgress] = useState<PatchProgress | null>(null);
  const [error, setError] = useState("");
  const [checkingTfg, setCheckingTfg] = useState(false);
  const [busy, setBusy] = useState(false);
  const [lastPatchCheck, setLastPatchCheck] = useState<string | null>(null);

  const [appVersion, setAppVersion] = useState("…");
  const [appUpdate, setAppUpdate] = useState<Update | null>(null);
  const [appUpdateStatus, setAppUpdateStatus] = useState("");
  const [appChecking, setAppChecking] = useState(false);
  const [appChecked, setAppChecked] = useState(false);
  const [appCheckFailed, setAppCheckFailed] = useState(false);
  const [appInstalling, setAppInstalling] = useState(false);
  const [showUpdateToast, setShowUpdateToast] = useState(false);
  const notifiedAppVersion = useRef<string | null>(null);

  useEffect(() => {
    const subscription = listen<PatchProgress>("patch-progress", (event) => {
      setProgress(event.payload);
    });
    return () => {
      void subscription.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    void getVersion().then(setAppVersion).catch(() => setAppVersion("unknown"));
    void checkAppUpdater(true);
    const interval = window.setInterval(
      () => void checkAppUpdater(true),
      APP_UPDATE_INTERVAL_MS
    );
    return () => window.clearInterval(interval);
  }, []);

  useEffect(() => {
    if (!root) {
      setState(null);
      setTfg(null);
      return;
    }

    void tfgApi.state(root).then(setState).catch(() => setState(null));
    void refreshTfg(true);

    const interval = window.setInterval(
      () => void refreshTfg(true),
      TFG_UPDATE_INTERVAL_MS
    );
    return () => window.clearInterval(interval);
  }, [root]);

  const status = useMemo(() => {
    if (!root) return "Select TFG folder";
    if (checkingTfg) return "Checking releases";
    if (!tfg) return "TFG not inspected";
    if (tfg.plan.items.length === 0) return "TFG installation current";
    if (tfg.plan.alreadyApplied) return "TFG installation drift";
    return "TFG update available";
  }, [root, checkingTfg, tfg]);

  async function checkAppUpdater(silent: boolean) {
    if (!silent) {
      setAppChecking(true);
      setAppUpdateStatus("");
    }

    try {
      const update = await check({ timeout: 15_000 });
      setAppUpdate(update);
      setAppChecked(true);
      setAppCheckFailed(false);

      if (update) {
        if (!silent || shouldNotifyAppUpdate(update.version, notifiedAppVersion.current)) {
          setShowUpdateToast(true);
        }
        notifiedAppVersion.current = update.version;
        if (!silent) {
          setAppUpdateStatus("Modpack Manager " + update.version + " is available.");
        }
      } else {
        setShowUpdateToast(false);
        if (!silent) {
          setAppUpdateStatus("Modpack Manager is up to date.");
        }
      }
    } catch (cause) {
      setAppChecked(true);
      setAppCheckFailed(true);
      if (!silent) {
        setAppUpdateStatus("Update check failed: " + errorMessage(cause));
      }
    } finally {
      if (!silent) setAppChecking(false);
    }
  }

  async function installAppUpdate() {
    if (!appUpdate || appInstalling) return;
    setAppInstalling(true);
    setShowUpdateToast(true);
    setAppUpdateStatus("Downloading Modpack Manager " + appUpdate.version + "…");

    let downloaded = 0;
    let total = 0;

    try {
      await appUpdate.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
          setAppUpdateStatus(
            total > 0
              ? "Downloading update: 0 / " + Math.round(total / 1024) + " KiB"
              : "Downloading update…"
          );
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setAppUpdateStatus(
            total > 0
              ? "Downloading update: " + Math.round(downloaded / 1024) + " / " + Math.round(total / 1024) + " KiB"
              : "Downloaded " + Math.round(downloaded / 1024) + " KiB"
          );
        } else if (event.event === "Finished") {
          setAppUpdateStatus("Update verified and installed. Restarting…");
        }
      });
      await relaunch();
    } catch (cause) {
      setAppUpdateStatus("Update failed: " + errorMessage(cause));
      setAppInstalling(false);
    }
  }

  async function useFolder(path = folderDraft) {
    const next = path.trim();
    if (!next) return;
    setFolderDraft(next);
    setRoot(next);
    localStorage.setItem(ROOT_KEY, next);
    setTfg(null);
    setResult(null);
    setError("");
  }

  async function chooseRoot() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select your TFG modpack folder"
    });

    if (typeof selected === "string") {
      await useFolder(selected);
    }
  }

  async function refreshTfg(silent: boolean) {
    if (!root.trim()) return;

    if (!silent) setCheckingTfg(true);
    setResult(null);

    try {
      const [next, nextState] = await Promise.all([
        tfgApi.plan(root.trim()),
        tfgApi.state(root.trim())
      ]);
      setTfg(next);
      setState(nextState);
      setLastPatchCheck(new Date().toLocaleTimeString());
      setError("");
    } catch (cause) {
      setTfg(null);
      if (!silent) setError(errorMessage(cause));
    } finally {
      if (!silent) setCheckingTfg(false);
    }
  }

  async function apply() {
    if (!tfg || busy) return;
    setBusy(true);
    setError("");
    setResult(null);

    try {
      const applied = await tfgApi.apply(root.trim(), tfg.patchVersion);
      setResult(applied);
      setState(applied.state);
      setTfg(await tfgApi.plan(root.trim()));
      setLastPatchCheck(new Date().toLocaleTimeString());
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  }

  return (
    <main className="shell">
      {appUpdate && showUpdateToast && (
        <aside className="updateToast" role="status" aria-live="polite">
          <button
            className="toastClose"
            aria-label="Dismiss update notification"
            onClick={() => setShowUpdateToast(false)}
          >
            ×
          </button>
          <div className="toastEyebrow">APP UPDATE AVAILABLE</div>
          <strong>Modpack Manager {appUpdate.version}</strong>
          <span>
            Installed: {appVersion}. The updater verifies the signed release before installing it.
          </span>
          {appUpdateStatus && <small>{appUpdateStatus}</small>}
          <button
            className="primary compact"
            disabled={appInstalling}
            onClick={installAppUpdate}
          >
            {appInstalling ? "Installing…" : "Update and restart"}
          </button>
        </aside>
      )}

      <header className="hero">
        <div className="heroCopy">
          <div className="eyebrow">TFG DESKTOP CONTROL</div>
          <h1>Modpack Manager</h1>
          <p>
            Manage TFG content updates and Modpack Manager releases separately. TFG changes stay
            scoped to the selected Minecraft installation; application updates are global.
          </p>
        </div>
        <div className="heroStatus">
          <small>TFG status</small>
          <div className="status">{status}</div>
        </div>
      </header>

      <div className="overviewGrid">
        <section className="panel tfgFolderPanel">
          <div className="heading"><span>01</span><h2>TFG modpack folder</h2></div>
          <p className="sectionCopy">
            Enter the TFG game directory itself — the folder containing <code>mods</code>,
            <code>config</code> and <code>kubejs</code>. The manager validates those markers before
            changing anything.
          </p>
          <label>
            TFG folder path
            <div className="pathRow">
              <input
                value={folderDraft}
                onChange={(event) => setFolderDraft(event.target.value)}
                placeholder={"C:\\Games\\PrismLauncher\\instances\\TFG\\.minecraft"}
                spellCheck={false}
              />
              <button className="secondary" onClick={chooseRoot}>Browse</button>
              <button
                className="primary compact"
                disabled={!folderDraft.trim()}
                onClick={() => void useFolder()}
              >
                Use folder
              </button>
            </div>
          </label>

          <div className="toolbar">
            <button
              className="secondary"
              disabled={!root || checkingTfg}
              onClick={() => void refreshTfg(false)}
            >
              {checkingTfg ? "Checking…" : "Check TFG updates"}
            </button>
            {lastPatchCheck && <span className="muted">TFG checked {lastPatchCheck}</span>}
          </div>
        </section>

        <section className="panel appPanel">
          <div className="heading"><span>APP</span><h2>Application updates</h2></div>
          <p className="sectionCopy">
            Modpack Manager updates are independent from the TFG installation. The app checks once
            on startup and then every six hours while it remains open.
          </p>

          <div className="appMeta">
            <div>
              <small>Installed version</small>
              <strong>{appVersion}</strong>
            </div>
            <div>
              <small>Update status</small>
              <strong>
                {!appChecked || appChecking
                  ? "Checking…"
                  : appCheckFailed
                    ? "Check unavailable"
                    : appUpdate
                      ? appUpdate.version + " available"
                      : "Up to date"}
              </strong>
            </div>
          </div>

          {appUpdate && (
            <div className="appUpdateAvailable">
              <small>New Modpack Manager release</small>
              <strong>{appUpdate.version}</strong>
              <span>{appUpdate.body || "A newer signed application release is ready to install."}</span>
            </div>
          )}

          <div className="appActions">
            <button
              className="secondary"
              disabled={appChecking}
              onClick={() => void checkAppUpdater(false)}
            >
              {appChecking ? "Checking…" : "Check for app update"}
            </button>
            {appUpdate && (
              <button
                className="primary compact"
                disabled={appInstalling}
                onClick={installAppUpdate}
              >
                {appInstalling ? "Installing…" : "Update to " + appUpdate.version}
              </button>
            )}
          </div>

          {appUpdateStatus && <div className="appUpdateMessage">{appUpdateStatus}</div>}
        </section>
      </div>

      <div className="contentGrid">
        <section className="panel modsPanel">
          <div className="heading"><span>02</span><h2>Managed TFG mods</h2></div>
          {!tfg ? (
            <div className="empty">
              Select a valid TFG folder to resolve the latest Forge 1.20.1 releases.
            </div>
          ) : (
            <div className="modGrid">
              {tfg.mods.map((managed) => (
                <article className="modCard" key={managed.id}>
                  <div className="modTitle">
                    <strong>{managed.name}</strong>
                    <span className={managed.targets.includes("server") ? "targetBadge" : "targetBadge clientOnly"}>
                      {targetLabel(managed.targets)}
                    </span>
                  </div>
                  <div className="modVersion">{managed.version}</div>
                  <small>{managed.source}</small>
                  <code>{managed.fileName}</code>
                </article>
              ))}
            </div>
          )}
          {tfg && (
            <div className="notice info">
              OpenUI and Create: Precise Controls are installed on clients only and removed from
              servers. Create Horse Power - CE replaces the original Create Horse Power JAR. Every
              managed mod removes older matching JARs before installation.
            </div>
          )}
        </section>

        <section className="panel diffPanel">
          <div className="heading"><span>03</span><h2>TFG installation diff</h2></div>
          {!tfg ? (
            <div className="empty">No filesystem changes are planned until the TFG folder is validated.</div>
          ) : (
            <>
              <div className="summary">
                <div><small>Profile</small><strong>Forge 1.20.1</strong></div>
                <div><small>Resolved set</small><strong>{tfg.patchVersion}</strong></div>
                <div><small>Installed set</small><strong>{state?.manifestVersion ?? "—"}</strong></div>
                <div><small>Filesystem changes</small><strong>{tfg.plan.items.length}</strong></div>
              </div>

              {tfg.plan.warnings.map((warning) => (
                <div className="notice warning" key={warning}>{warning}</div>
              ))}

              {tfg.plan.items.length === 0 && (
                <div className="notice success">
                  No managed filesystem changes are needed. This installation matches the resolved client patch.
                </div>
              )}

              <div className="operations">
                {tfg.plan.items.map((item, index) => (
                  <div className="operation" key={item.kind + item.path + index}>
                    <strong>{item.kind}</strong>
                    <span>{item.path}</span>
                    <small>{item.detail}</small>
                  </div>
                ))}
              </div>

              <button className="primary" disabled={busy || tfg.plan.items.length === 0} onClick={apply}>
                {busy
                  ? "Patching TFG…"
                  : tfg.plan.alreadyApplied
                    ? "Repair installation drift"
                    : "Apply TFG managed changes"}
              </button>
            </>
          )}
        </section>
      </div>

      {(progress || error || result) && (
        <section className="panel activityPanel">
          <div className="heading"><span>04</span><h2>Activity</h2></div>
          {progress && (
            <div className="progress">
              <div><strong>{progress.message}</strong><span>{progress.current}/{progress.total}</span></div>
              <progress
                max={Math.max(progress.total, 1)}
                value={Math.min(progress.current, Math.max(progress.total, 1))}
              />
            </div>
          )}
          {error && <div className="notice error">{error}</div>}
          {result && (
            <div className="notice success">
              TFG patch applied. {result.changedPaths} paths changed
              {result.backupPath ? "; backup: " + result.backupPath : ""}.
            </div>
          )}
        </section>
      )}

      <footer>
        Modpack Manager {appVersion} · TFG releases auto-check every 30 minutes · app releases check on startup and every 6 hours
      </footer>
    </main>
  );
}
