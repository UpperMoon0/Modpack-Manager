import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { patchApi } from "./api";
import { patchStatus } from "./status";
import type { ApplyResult, PatchPlan, PatchProgress, PatchState } from "./types";

const MANIFEST_KEY = "modpack-manager.manifest";
const ROOT_KEY = "modpack-manager.root";

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export default function App() {
  const [manifestSource, setManifestSource] = useState(
    () => localStorage.getItem(MANIFEST_KEY) ?? ""
  );
  const [root, setRoot] = useState(() => localStorage.getItem(ROOT_KEY) ?? "");
  const [plan, setPlan] = useState<PatchPlan | null>(null);
  const [state, setState] = useState<PatchState | null>(null);
  const [result, setResult] = useState<ApplyResult | null>(null);
  const [progress, setProgress] = useState<PatchProgress | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const subscription = listen<PatchProgress>("patch-progress", (event) => {
      setProgress(event.payload);
    });
    return () => {
      void subscription.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    if (!root) {
      setState(null);
      return;
    }
    void patchApi.state(root).then(setState).catch(() => setState(null));
  }, [root]);

  const status = useMemo(
    () => patchStatus(Boolean(result), Boolean(plan), Boolean(plan?.alreadyApplied)),
    [plan, result]
  );

  async function chooseRoot() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select modpack game directory"
    });
    if (typeof selected === "string") {
      setRoot(selected);
      localStorage.setItem(ROOT_KEY, selected);
      setPlan(null);
      setResult(null);
    }
  }

  async function inspect() {
    if (!manifestSource.trim() || !root.trim() || busy) return;
    setBusy(true);
    setError("");
    setResult(null);
    try {
      const next = await patchApi.plan(manifestSource.trim(), root.trim());
      setPlan(next);
      localStorage.setItem(MANIFEST_KEY, manifestSource.trim());
      localStorage.setItem(ROOT_KEY, root.trim());
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function apply() {
    if (!plan || busy) return;
    setBusy(true);
    setError("");
    setResult(null);
    try {
      const applied = await patchApi.apply(manifestSource.trim(), root.trim());
      setResult(applied);
      setState(applied.state);
      setPlan(await patchApi.plan(manifestSource.trim(), root.trim()));
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
            Re-apply verified custom mods, scripts and config overrides after an upstream
            modpack update. The same patch manifest is used by the server and every player.
          </p>
        </div>
        <div className="status">{status}</div>
      </header>

      <section className="panel">
        <div className="heading"><span>01</span><h2>Patch source</h2></div>
        <label>
          Manifest URL or local JSON file
          <input
            value={manifestSource}
            onChange={(event) => {
              setManifestSource(event.target.value);
              setPlan(null);
              setResult(null);
            }}
            placeholder="https://example.com/tfg/patch.json"
            spellCheck={false}
          />
        </label>
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
        <button className="primary" disabled={busy || !manifestSource.trim() || !root.trim()} onClick={inspect}>
          {busy ? "Checking…" : "Inspect patch"}
        </button>
      </section>

      <section className="panel">
        <div className="heading"><span>02</span><h2>Change plan</h2></div>
        {!plan ? (
          <div className="empty">Nothing will be changed until a patch plan has been inspected.</div>
        ) : (
          <>
            <div className="summary">
              <div><small>Patch</small><strong>{plan.manifestName}</strong></div>
              <div><small>Version</small><strong>{plan.manifestVersion}</strong></div>
              <div><small>Installed</small><strong>{state?.manifestVersion ?? "—"}</strong></div>
              <div><small>Operations</small><strong>{plan.items.length}</strong></div>
            </div>

            {plan.warnings.map((warning) => <div className="notice warning" key={warning}>{warning}</div>)}

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
              {plan.alreadyApplied ? "Re-apply patch" : "Apply patch"}
            </button>
          </>
        )}
      </section>

      {(progress || error || result) && (
        <section className="panel">
          <div className="heading"><span>03</span><h2>Activity</h2></div>
          {progress && (
            <div className="progress">
              <div><strong>{progress.message}</strong><span>{progress.current}/{progress.total}</span></div>
              <progress max={Math.max(progress.total, 1)} value={Math.min(progress.current, Math.max(progress.total, 1))} />
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

      <footer>The native Rust backend owns all filesystem writes and rollback logic.</footer>
    </main>
  );
}
