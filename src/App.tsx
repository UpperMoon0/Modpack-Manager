import { useEffect, useMemo, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { patchApi } from "./api";
import {
  APP_UPDATE_INTERVAL_MS,
  DEFAULT_PATCH_CHANNEL,
  PATCH_CHANNEL_KEY,
  ROOT_KEY,
  patchPollIntervalMs
} from "./channel";
import { patchStatus } from "./status";
import type {
  ApplyResult,
  PatchPlan,
  PatchProgress,
  PatchState,
  ResolvedPatchChannel
} from "./types";

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export default function App() {
  const storedChannel = localStorage.getItem(PATCH_CHANNEL_KEY) ?? DEFAULT_PATCH_CHANNEL;
  const [channelSource, setChannelSource] = useState(storedChannel);
  const [channelDraft, setChannelDraft] = useState(storedChannel);
  const [root, setRoot] = useState(() => localStorage.getItem(ROOT_KEY) ?? "");
  const [channel, setChannel] = useState<ResolvedPatchChannel | null>(null);
  const [plan, setPlan] = useState<PatchPlan | null>(null);
  const [state, setState] = useState<PatchState | null>(null);
  const [result, setResult] = useState<ApplyResult | null>(null);
  const [progress, setProgress] = useState<PatchProgress | null>(null);
  const [error, setError] = useState("");
  const [channelBusy, setChannelBusy] = useState(false);
  const [appVersion, setAppVersion] = useState("…");
  const [appUpdate, setAppUpdate] = useState<Update | null>(null);
  const [appUpdateStatus, setAppUpdateStatus] = useState("");
  const [appChecking, setAppChecking] = useState(false);
  const [appInstalling, setAppInstalling] = useState(false);
  const [busy, setBusy] = useState(false);
  const [lastPatchCheck, setLastPatchCheck] = useState<string | null>(null);

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
    const startup = window.setTimeout(() => void checkAppUpdater(true), 1500);
    const interval = window.setInterval(
      () => void checkAppUpdater(true),
      APP_UPDATE_INTERVAL_MS
    );
    return () => {
      window.clearTimeout(startup);
      window.clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    if (!root) {
      setState(null);
      return;
    }
    localStorage.setItem(ROOT_KEY, root);
    void patchApi.state(root).then(setState).catch(() => setState(null));
  }, [root]);

  useEffect(() => {
    if (!channelSource.trim()) return;

    void refreshPatchChannel(true);
    const interval = window.setInterval(
      () => void refreshPatchChannel(true),
      patchPollIntervalMs(channel?.checkIntervalMinutes ?? 30)
    );

    return () => window.clearInterval(interval);
  }, [channelSource, root, channel?.checkIntervalMinutes]);

  const status = useMemo(
    () => patchStatus(Boolean(result), Boolean(plan), Boolean(plan?.alreadyApplied)),
    [plan, result]
  );

  async function checkAppUpdater(silent: boolean) {
    if (!silent) {
      setAppChecking(true);
      setAppUpdateStatus("");
    }

    try {
      const update = await check({ timeout: 15_000 });
      setAppUpdate(update);
      if (!silent && !update) {
        setAppUpdateStatus("Modpack Manager is up to date.");
      }
    } catch (cause) {
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

  async function connectChannel() {
    const next = channelDraft.trim();
    if (!next) return;
    localStorage.setItem(PATCH_CHANNEL_KEY, next);
    setChannelSource(next);
    setChannel(null);
    setPlan(null);
    setResult(null);
    setError("");
  }

  async function refreshPatchChannel(silent: boolean) {
    if (!channelSource.trim()) return;

    if (!silent) setChannelBusy(true);
    try {
      const nextChannel = await patchApi.channel(channelSource.trim());
      setChannel(nextChannel);
      setLastPatchCheck(new Date().toLocaleTimeString());

      if (root.trim()) {
        const [nextState, nextPlan] = await Promise.all([
          patchApi.state(root.trim()),
          patchApi.plan(nextChannel.manifestSource, root.trim())
        ]);
        setState(nextState);
        setPlan(nextPlan);
      }
      if (!silent) setError("");
    } catch (cause) {
      if (!silent) setError(errorMessage(cause));
    } finally {
      if (!silent) setChannelBusy(false);
    }
  }

  async function chooseRoot() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select modpack game directory"
    });
    if (typeof selected === "string") {
      setRoot(selected);
      setPlan(null);
      setResult(null);
    }
  }

  async function apply() {
    if (!plan || !channel || busy) return;
    setBusy(true);
    setError("");
    setResult(null);
    try {
      const applied = await patchApi.apply(channel.manifestSource, root.trim());
      setResult(applied);
      setState(applied.state);
      setPlan(await patchApi.plan(channel.manifestSource, root.trim()));
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  }

  return (
    <main className="shell">
      <header className="hero">
        <div>
          <div className="eyebrow">PATCH CONTROL</div>
          <h1>Modpack Manager</h1>
          <p>
            Follow a remote release channel once. New modpack patches are discovered automatically,
            while the manager itself updates independently through signed application releases.
          </p>
        </div>
        <div className="status">{status}</div>
      </header>

      {appUpdate && (
        <section className="updateBanner">
          <div>
            <strong>Modpack Manager {appUpdate.version} is available</strong>
            <span>{appUpdate.body || "Current version: " + appVersion}</span>
            {appUpdateStatus && <small>{appUpdateStatus}</small>}
          </div>
          <button className="primary compact" disabled={appInstalling} onClick={installAppUpdate}>
            {appInstalling ? "Installing…" : "Install update"}
          </button>
        </section>
      )}

      <section className="panel">
        <div className="heading"><span>01</span><h2>Update channel</h2></div>
        <p className="sectionCopy">
          Use one stable channel URL from GitHub Releases, raw GitHub, a CDN, or any HTTPS host.
          The channel points to the newest patch manifest, so future modpack versions require no new app build.
        </p>
        <label>
          Patch channel URL
          <div className="pathRow">
            <input
              value={channelDraft}
              onChange={(event) => setChannelDraft(event.target.value)}
              placeholder="https://github.com/owner/tfg-patches/releases/latest/download/channel.json"
              spellCheck={false}
            />
            <button className="secondary" disabled={!channelDraft.trim()} onClick={connectChannel}>
              Connect
            </button>
          </div>
        </label>

        {channel && (
          <div className="channelCard">
            <div><small>Channel</small><strong>{channel.name}</strong></div>
            <div><small>Latest patch</small><strong>{channel.version}</strong></div>
            <div><small>Checks</small><strong>Every {channel.checkIntervalMinutes} min</strong></div>
            <div><small>Last check</small><strong>{lastPatchCheck ?? "—"}</strong></div>
            {channel.notes && <p>{channel.notes}</p>}
          </div>
        )}

        <div className="toolbar">
          <button
            className="secondary"
            disabled={!channelSource.trim() || channelBusy}
            onClick={() => void refreshPatchChannel(false)}
          >
            {channelBusy ? "Checking…" : "Check patch channel now"}
          </button>
          <button
            className="secondary"
            disabled={appChecking}
            onClick={() => void checkAppUpdater(false)}
          >
            {appChecking ? "Checking…" : "Check app update"}
          </button>
          {appUpdateStatus && !appUpdate && <span className="muted">{appUpdateStatus}</span>}
        </div>
      </section>

      <section className="panel">
        <div className="heading"><span>02</span><h2>Installation</h2></div>
        <label>
          Modpack game directory
          <div className="pathRow">
            <input
              value={root}
              onChange={(event) => {
                setRoot(event.target.value);
                setPlan(null);
                setResult(null);
              }}
              placeholder="C:\Games\PrismLauncher\instances\TFG\.minecraft"
              spellCheck={false}
            />
            <button className="secondary" onClick={chooseRoot}>Browse</button>
          </div>
        </label>
      </section>

      <section className="panel">
        <div className="heading"><span>03</span><h2>Change plan</h2></div>
        {!plan ? (
          <div className="empty">
            Connect an update channel and select the modpack directory. The manager will check the
            channel automatically and show the exact filesystem plan here.
          </div>
        ) : (
          <>
            <div className="summary">
              <div><small>Patch</small><strong>{plan.manifestName}</strong></div>
              <div><small>Available</small><strong>{plan.manifestVersion}</strong></div>
              <div><small>Installed</small><strong>{state?.manifestVersion ?? "—"}</strong></div>
              <div><small>Operations</small><strong>{plan.items.length}</strong></div>
            </div>

            {plan.warnings.map((warning) => (
              <div className="notice warning" key={warning}>{warning}</div>
            ))}

            <div className="operations">
              {plan.items.map((item, index) => (
                <div className="operation" key={item.kind + item.path + index}>
                  <strong>{item.kind}</strong>
                  <span>{item.path}</span>
                  <small>{item.detail}</small>
                </div>
              ))}
            </div>

            <button className="primary" disabled={busy || plan.items.length === 0} onClick={apply}>
              {busy ? "Applying…" : plan.alreadyApplied ? "Re-apply patch" : "Update modpack"}
            </button>
          </>
        )}
      </section>

      {(progress || error || result) && (
        <section className="panel">
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
              Patch applied. {result.changedPaths} paths changed
              {result.backupPath ? "; backup: " + result.backupPath : ""}.
            </div>
          )}
        </section>
      )}

      <footer>
        Modpack Manager {appVersion} · patch channels auto-check in the background · app releases check every 6 hours
      </footer>
    </main>
  );
}
