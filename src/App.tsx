import { useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { tfgApi } from "./api";
import { APP_UPDATE_INTERVAL_MS, ROOT_KEY } from "./channel";
import { shouldNotifyAppUpdate } from "./appUpdate";
import { Icon } from "./Icon";
import { ACTIVITY_KEY, PACKS_KEY, readActivity, readLibrary, rememberPack, rootKey, type ActivityEntry } from "./library";
import type { FreshInstallPlan, FreshInstallResult, PatchProgress, PatchState, TfgPlanResponse } from "./types";

type Page = "packs" | "activity" | "settings" | "details";
type Tab = "overview" | "mods" | "history";
type WizardStep = "choose" | "existing" | "location" | "review" | "installing" | "finish";
interface PackView { data?: TfgPlanResponse; state?: PatchState | null; checkedAt?: string; error?: string }
const POLL_INTERVAL = 30 * 60 * 1000;
const errorMessage = (error: unknown) => error instanceof Error ? error.message : String(error);
const dateLabel = (value?: string | null) => value ? new Date(value).toLocaleString() : "Not checked yet";
function packLabel(root: string) {
 const parts = root.replace(/\\/g, "/").replace(/\/+$/, "").split("/");
 const leaf = parts.pop();
 return leaf === ".minecraft" || leaf === "minecraft" ? parts.pop() || "TFG" : leaf || "TFG";
}
function save(key: string, value: unknown) { localStorage.setItem(key, JSON.stringify(value)); }

export default function App() {
 const [packs, setPacks] = useState(() => readLibrary(localStorage));
 const [selectedRoot, setSelectedRoot] = useState(() => localStorage.getItem(ROOT_KEY) ?? "");
 const [page, setPage] = useState<Page>("packs");
 const [tab, setTab] = useState<Tab>("overview");
 const [views, setViews] = useState<Record<string, PackView>>({});
 const [loading, setLoading] = useState<string[]>([]);
 const [activity, setActivity] = useState(() => readActivity(localStorage));
 const [error, setError] = useState("");
 const [progress, setProgress] = useState<PatchProgress | null>(null);
 const [operation, setOperation] = useState<{ root: string; title: string } | null>(null);
 const operationRef = useRef<typeof operation>(null);
 const generations = useRef<Record<string, number>>({});
 const [wizardOpen, setWizardOpen] = useState(false);
 const [step, setStep] = useState<WizardStep>("choose");
 const [folder, setFolder] = useState("");
 const [replaceRoot, setReplaceRoot] = useState<string | undefined>();
 const [wizardError, setWizardError] = useState("");
 const [validating, setValidating] = useState(false);
 const [installPlan, setInstallPlan] = useState<FreshInstallPlan | null>(null);
 const [installed, setInstalled] = useState<FreshInstallResult | null>(null);
 const dialog = useRef<HTMLDialogElement>(null);
 const [query, setQuery] = useState("");
 const [appVersion, setAppVersion] = useState("…");
 const [appUpdate, setAppUpdate] = useState<Update | null>(null);
 const [appChecking, setAppChecking] = useState(false);
 const [appChecked, setAppChecked] = useState(false);
 const [appCheckFailed, setAppCheckFailed] = useState(false);
 const [appInstalling, setAppInstalling] = useState(false);
 const appInstallingRef = useRef(false);
 const [appUpdateStatus, setAppUpdateStatus] = useState("");
 const [showUpdateToast, setShowUpdateToast] = useState(false);
 const notifiedAppVersion = useRef<string | null>(null);
 const busy = Boolean(operation) || appInstalling;
 const selectedPack = packs.find((pack) => rootKey(pack.root) === rootKey(selectedRoot));
 const view = views[rootKey(selectedRoot)];
 const tfg = view?.data;
 const packRoots = packs.map((pack) => pack.root).join("\n");

 useEffect(() => {
  try { save(PACKS_KEY, packs); } catch { setError("Your pack list could not be saved. Check free disk space."); }
 }, [packs]);
 useEffect(() => {
  try { save(ACTIVITY_KEY, activity); } catch { setError("Activity could not be saved. Check free disk space."); }
 }, [activity]);
 useEffect(() => {
  const subscription = listen<PatchProgress>("patch-progress", (event) => {
   if (operationRef.current) setProgress(event.payload);
  });
  return () => { void subscription.then((unlisten) => unlisten(), () => {}); };
 }, []);
 useEffect(() => {
  void getVersion().then(setAppVersion).catch(() => setAppVersion("unknown"));
  void checkAppUpdater(true);
  const timer = window.setInterval(() => void checkAppUpdater(true), APP_UPDATE_INTERVAL_MS);
  return () => window.clearInterval(timer);
 }, []);
 useEffect(() => {
  const roots = packRoots ? packRoots.split("\n") : [];
  for (const root of roots) void refreshPack(root, true);
  const timer = window.setInterval(() => {
   if (!operationRef.current && !appInstallingRef.current) {
    for (const root of roots) void refreshPack(root, true);
   }
  }, POLL_INTERVAL);
  return () => window.clearInterval(timer);
 }, [packRoots]);
 useEffect(() => {
  if (wizardOpen && !dialog.current?.open) dialog.current?.showModal();
  if (!wizardOpen && dialog.current?.open) dialog.current?.close();
 }, [wizardOpen]);

 function log(root: string, title: string, status: ActivityEntry["status"], detail: string, backupPath?: string | null) {
  const entry: ActivityEntry = { id: crypto.randomUUID(), root, title, status,
   at: new Date().toISOString(), detail: detail.slice(0, 8000), backupPath };
  setActivity((previous) => [entry, ...previous].slice(0, 200));
 }
 function selectPack(root: string, showDetails = false) {
  setSelectedRoot(root);
  localStorage.setItem(ROOT_KEY, root);
  setTab("overview");
  setQuery("");
  if (showDetails) setPage("details");
 }
 function remember(root: string, state: PatchState | null, previousRoot?: string) {
  setPacks((previous) => rememberPack(previous, root, state, previousRoot));
 }
 async function refreshPack(root: string, silent: boolean) {
  if (operationRef.current || appInstallingRef.current) return;
  const key = rootKey(root);
  const generation = (generations.current[key] ?? 0) + 1;
  generations.current[key] = generation;
  setLoading((previous) => [...new Set([...previous, key])]);
  try {
   const [data, state] = await Promise.all([tfgApi.plan(root), tfgApi.state(root)]);
   if (generations.current[key] !== generation) return;
   setViews((previous) => ({ ...previous, [key]: { data, state, checkedAt: new Date().toISOString() } }));
   remember(root, state);
   if (!silent) log(root, "Checked for updates", "success",
    data.plan.items.length ? `${data.patchVersion} is available. ${data.plan.items.length} changes.` : "Your pack is up to date.");
  } catch (cause) {
   if (generations.current[key] !== generation) return;
   const message = errorMessage(cause);
   setViews((previous) => ({ ...previous, [key]: { state: previous[key]?.state, error: message } }));
   if (!silent) log(root, "Update check failed", "error", message);
  } finally {
   if (generations.current[key] === generation) setLoading((previous) => previous.filter((item) => item !== key));
  }
 }
 function invalidateCheck(root: string) {
  const key = rootKey(root);
  generations.current[key] = (generations.current[key] ?? 0) + 1;
  setLoading((previous) => previous.filter((item) => item !== key));
 }
 function beginOperation(root: string, title: string) {
  if (operationRef.current || appInstallingRef.current) return false;
  const next = { root, title };
  operationRef.current = next;
  setOperation(next);
  setProgress(null);
  setError("");
  invalidateCheck(root);
  return true;
 }
 function finishOperation() { operationRef.current = null; setOperation(null); setProgress(null); }
 async function apply(root: string, expectedVersion: string) {
  if (!beginOperation(root, "Updating TFG")) return;
  try {
   const result = await tfgApi.apply(root, expectedVersion);
   remember(root, result.state);
   log(root, `Updated to ${result.state.manifestVersion}`, "success", `${result.changedPaths} files changed.`, result.backupPath);
  } catch (cause) {
   const message = errorMessage(cause);
   setError("The update failed. See Activity for details.");
   log(root, "TFG update failed", "error", message);
  } finally {
   finishOperation();
   await refreshPack(root, true);
  }
 }
 async function openFolder(root: string) {
  try { await tfgApi.openFolder(root); } catch (cause) { setError(errorMessage(cause)); }
 }
 function startWizard(mode: WizardStep = "choose", oldRoot?: string) {
  if (busy) return;
  setFolder(oldRoot ?? ""); setStep(mode); setReplaceRoot(oldRoot);
  setWizardError(""); setInstallPlan(null); setInstalled(null); setWizardOpen(true);
 }
 async function browseFolder() {
  try {
   const selected = await open({ directory: true, multiple: false,
    title: step === "existing" ? "Select the game folder inside your TFG instance" : "Choose an empty folder for TFG" });
   if (typeof selected === "string") { setFolder(selected); setInstallPlan(null); setWizardError(""); }
  } catch (cause) { setWizardError(errorMessage(cause)); }
 }
 async function validateFolder() {
  if (validating || busy || !folder.trim()) return;
  setValidating(true); setWizardError("");
  try {
   if (step === "existing") {
    const root = folder.trim();
    const [data, state] = await Promise.all([tfgApi.plan(root), tfgApi.state(root)]);
    // A replaced folder must not be re-added by a check that began earlier.
    invalidateCheck(root);
    if (replaceRoot) invalidateCheck(replaceRoot);
    remember(root, state, replaceRoot);
    setViews((previous) => ({ ...previous, [rootKey(root)]: { data, state, checkedAt: new Date().toISOString() } }));
    selectPack(root, true); setWizardOpen(false);
   } else {
    setInstallPlan(await tfgApi.installPlan(folder.trim())); setStep("review");
   }
  } catch (cause) { setWizardError(errorMessage(cause)); } finally { setValidating(false); }
 }
 async function freshInstall() {
  let completedRoot: string | undefined;
  if (!installPlan || !beginOperation(installPlan.destination, "Installing TFG")) return;
  setStep("installing"); setWizardError("");
  try {
   const result = await tfgApi.install(installPlan.destination, installPlan.version);
   setInstalled(result); setStep("finish"); completedRoot = result.minecraftRoot;
   const state = await tfgApi.state(result.minecraftRoot).catch(() => null);
   remember(result.minecraftRoot, state);
   selectPack(result.minecraftRoot);
   log(result.minecraftRoot, `Installed TFG ${result.version}`, "success",
    `Import ${result.archivePath} through Prism Launcher → Add Instance → Import. After importing, add the playable instance's game folder to Manager.`, null);
  } catch (cause) {
   const message = errorMessage(cause); setWizardError(message); setStep("review");
   log(installPlan.destination, "TFG installation failed", "error", message);
   setError("The installation failed. See Activity for details.");
  } finally { finishOperation(); if (completedRoot) await refreshPack(completedRoot, true); }
 }
 async function checkAppUpdater(silent: boolean) {
  if (!silent) { setAppChecking(true); setAppUpdateStatus(""); }
  try {
   const update = await check({ timeout: 15_000 });
   setAppUpdate(update); setAppChecked(true); setAppCheckFailed(false);
   if (update) {
    if (shouldNotifyAppUpdate(update.version, notifiedAppVersion.current)) setShowUpdateToast(true);
    notifiedAppVersion.current = update.version;
    if (!silent) setAppUpdateStatus(`Modpack Manager ${update.version} is available.`);
   } else { setShowUpdateToast(false); if (!silent) setAppUpdateStatus("Modpack Manager is up to date."); }
  } catch (cause) {
   setAppChecked(true); setAppCheckFailed(true);
   if (!silent) setAppUpdateStatus("Check failed: " + errorMessage(cause));
  } finally { if (!silent) setAppChecking(false); }
 }
 async function installAppUpdate() {
  if (!appUpdate || operationRef.current || appInstallingRef.current) return;
  appInstallingRef.current = true; setAppInstalling(true); setShowUpdateToast(false);
  setAppUpdateStatus(`Downloading Modpack Manager ${appUpdate.version}…`);
  let downloaded = 0; let total = 0;
  try {
   await appUpdate.downloadAndInstall((event) => {
    if (event.event === "Started") total = event.data.contentLength ?? 0;
    if (event.event === "Progress") {
     downloaded += event.data.chunkLength;
     setAppUpdateStatus(total ? `Downloading app update — ${Math.min(100, Math.round(downloaded / total * 100))}%` : "Downloading app update…");
    }
    if (event.event === "Finished") setAppUpdateStatus("Update installed. Restarting…");
   });
   await relaunch();
  } catch (cause) {
   setAppUpdateStatus("Update failed: " + errorMessage(cause));
   log("", "Manager update failed", "error", errorMessage(cause));
   appInstallingRef.current = false; setAppInstalling(false);
  }
 }
 function packStatus(root: string) {
  if (operation && rootKey(operation.root) === rootKey(root)) return operation.title;
  if (loading.includes(rootKey(root))) return "Checking…";
  const next = views[rootKey(root)];
  if (next?.error) return "Check unavailable";
  if (!next?.data) return "Not checked yet";
  if (!next.data.plan.items.length) return "Up to date";
  return next.data.plan.alreadyApplied ? "Repair available" : "Update available";
 }
 const title = page === "packs" ? "My packs" : page === "activity" ? "Activity" : page === "settings" ? "Settings" : "TerraFirmaGreg";
 const filteredMods = tfg?.mods.filter((mod) => `${mod.name} ${mod.version} ${mod.source}`.toLowerCase().includes(query.toLowerCase())) ?? [];
 const packHistory = activity.filter((entry) => rootKey(entry.root) === rootKey(selectedRoot));
 function historyList(entries: ActivityEntry[]) {
  return entries.length ? <div className="activityList">{entries.map((entry) => <article className="activityEntry" key={entry.id}>
   <span className={`eventMark ${entry.status}`}>{entry.status === "success" ? <Icon name="check" /> : "!"}</span>
   <div><strong>{entry.title}</strong><p>{entry.root ? packLabel(entry.root) + " · " : ""}{dateLabel(entry.at)}</p>
    <details><summary>Details</summary><p className="eventDetail">{entry.detail}</p>
     {entry.backupPath && <p>Recovery backup: <code>{entry.backupPath}</code></p>}</details>
   </div>
  </article>)}</div> : <div className="emptyState compactEmpty"><Icon name="activity" /><h3>No activity yet</h3><p>Completed updates and any problems will appear here.</p></div>;
 }
 return <div className="appShell">
  <aside className="sidebar">
   <div className="brand"><span className="brandMark"><Icon name="packs" /></span><span>Modpack<br />Manager</span></div>
   <nav aria-label="Main navigation">
    {([ ["packs", "My packs", "packs"], ["activity", "Activity", "activity"], ["settings", "Settings", "settings"] ] as const).map(([destination, label, icon]) =>
     <button key={destination} className={(page === destination || (destination === "packs" && page === "details")) ? "navItem active" : "navItem"}
      aria-current={(page === destination || (destination === "packs" && page === "details")) ? "page" : undefined} onClick={() => setPage(destination)}>
      <Icon name={icon} />{label}{destination === "settings" && appUpdate && <span className="notificationDot" aria-label="App update available" />}
     </button>)}
   </nav>
   <div className="sidebarFooter"><span className="sidebarAvatar">M</span><div><strong>Modpack Manager</strong><small>Version {appVersion}</small></div></div>
  </aside>
  <div className="mainArea">
   <header className="topbar"><span>{title}</span><span className="topbarNote">TFG · NsTut</span></header>
   <main className="workspace">
    {error && <div className="notice error" role="alert"><span>{error}</span><button className="iconButton" aria-label="Dismiss message" onClick={() => setError("")}><Icon name="close" /></button></div>}
    {page === "packs" && <>
     <div className="pageHeading"><div><h1>My packs</h1><p>Your installations, ready when you are.</p></div><button className="primary" disabled={busy} onClick={() => startWizard()}><Icon name="plus" />Add pack</button></div>
     {!packs.length ? <div className="emptyState"><span className="emptyIcon"><Icon name="packs" /></span><h2>Your first pack starts here</h2><p>Install the complete TFG pack or connect an installation you already use.</p><button className="primary" disabled={busy} onClick={() => startWizard()}>Add your first pack</button></div>
      : <div className="packGrid">{packs.map((pack) => {
       const next = views[rootKey(pack.root)]; const status = packStatus(pack.root); const current = status === "Up to date";
       return <article className="packCard" key={rootKey(pack.root)}>
        <div className="packCover"><div className="packMonogram">TFG<span>NsTut edition</span></div><span className="coverTag">Forge · 1.20.1</span></div>
        <div className="packBody"><div className="packTitleRow"><div><h2>TerraFirmaGreg</h2><p>{packLabel(pack.root)} · NsTut</p></div>
         <details className="packMenu"><summary className="iconButton" aria-label={`Options for ${packLabel(pack.root)}`}><Icon name="more" /></summary><div className="menuPopover"><button onClick={() => void openFolder(pack.root)}><Icon name="folder" />Open folder</button><button disabled={busy} onClick={() => startWizard("existing", pack.root)}>Change folder</button></div></details></div>
         <div className="packVersion">Installed <strong>{pack.version ?? "Not recorded yet"}</strong></div>
         <div className={`packStatus ${current ? "current" : ""}`}><span className="statusDot" />{status}</div>
         <small className="lastChecked">{next?.checkedAt ? "Checked " + dateLabel(next.checkedAt) : ""}</small>
         <div className="packActions"><button className="secondary" disabled={busy || loading.includes(rootKey(pack.root))} onClick={() => void refreshPack(pack.root, false)}>Check for updates</button><button className="textButton" onClick={() => selectPack(pack.root, true)}>Details →</button></div>
        </div>
       </article>;
      })}</div>}
    </>}
    {page === "details" && selectedPack && <>
     <button className="backButton" onClick={() => setPage("packs")}><Icon name="arrow" />My packs</button>
     <div className="pageHeading detailHeading"><div><h1>TerraFirmaGreg <span className="editionBadge">NsTut</span></h1><p>{packLabel(selectedRoot)} · Forge 1.20.1</p></div><span className="statusPill">{packStatus(selectedRoot)}</span></div>
     <div className="tabs" role="tablist" aria-label="Pack details">{(["overview", "mods", "history"] as const).map((next) => <button role="tab" id={`tab-${next}`} aria-controls={`panel-${next}`} aria-selected={tab === next} className={tab === next ? "active" : ""} key={next} onClick={() => setTab(next)}>{next === "overview" ? "Overview" : next === "mods" ? "Mods" : "Update history"}</button>)}</div>
     <section id={`panel-${tab}`} role="tabpanel" aria-labelledby={`tab-${tab}`}>
      {tab === "overview" && <div className="detailStack">
       <div className="panel updatePanel"><div className="sectionHeading"><h2>{tfg?.plan.items.length ? (tfg.plan.alreadyApplied ? "Repair this installation" : "An update is ready") : "Pack updates"}</h2><button className="secondary" disabled={busy || loading.includes(rootKey(selectedRoot))} onClick={() => void refreshPack(selectedRoot, false)}>Check for updates</button></div>
        <div className="versionPair"><div><small>Installed version</small><strong>{selectedPack.version ?? "Not recorded yet"}</strong></div><span>→</span><div><small>Latest version</small><strong>{tfg?.patchVersion ?? "Checking…"}</strong></div></div>
        {view?.error ? <div className="notice error">{view.error}</div> : tfg && (!tfg.plan.items.length ? <p className="upToDate"><Icon name="check" />Your pack is up to date.</p> : <><p>{tfg.plan.items.length} changes are ready. Close Minecraft before updating.</p><button className="primary" disabled={busy} onClick={() => void apply(selectedRoot, tfg.patchVersion)}><Icon name="download" />{tfg.plan.alreadyApplied ? "Repair pack" : "Update pack"}</button></>)}
        {tfg?.plan.warnings.map((warning) => <div className="notice warning" key={warning}>{warning}</div>)}
        {Boolean(tfg?.plan.items.length) && <details className="updateDetails"><summary>View update details</summary><div className="operations">{tfg?.plan.items.map((item, index) => <div key={index}><strong>{item.kind === "migratePack" ? "Upgrade base pack" : item.kind}</strong><code>{item.path}</code><p>{item.detail}</p></div>)}</div></details>}
       </div>
       <div className="panel locationPanel"><div><h2>Installation location</h2><code>{selectedRoot}</code></div><div className="actions"><button className="secondary" onClick={() => void openFolder(selectedRoot)}><Icon name="folder" />Open folder</button><button className="textButton" disabled={busy} onClick={() => startWizard("existing", selectedRoot)}>Change folder</button></div></div>
      </div>}
      {tab === "mods" && <div className="panel"><div className="sectionHeading"><div><h2>Managed mods</h2><p>The custom mods maintained by this fork.</p></div><label className="searchLabel"><span className="srOnly">Search mods</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search mods…" /></label></div>
       {!tfg ? <p>Check the pack for updates to load its mods.</p> : <><div className="modList">{filteredMods.map((mod) => <details className="modRow" key={mod.id}><summary><strong>{mod.name}</strong><span>{mod.version}</span><span className="modTarget">{mod.targets.includes("server") ? "Client + server" : "Client only"}</span></summary><div className="modDetails"><span>{mod.source}</span><code>{mod.fileName}</code></div></details>)}</div>{!filteredMods.length && <p>No mods match your search.</p>}</>}
      </div>}
      {tab === "history" && <div className="panel"><h2>Update history</h2>{view?.state && <p className="muted">Last installed: {view.state.manifestVersion} · {dateLabel(view.state.appliedAt)}</p>}{historyList(packHistory)}</div>}
     </section>
    </>}
    {page === "activity" && <><div className="pageHeading"><div><h1>Activity</h1><p>Updates, installations, and recovery details.</p></div></div>{operation && <div className="panel activeOperation"><h2>{operation.title}</h2><p>{packLabel(operation.root)}</p><p>{progress?.message ?? "Preparing…"}</p><progress max={Math.max(progress?.total ?? 1, 1)} value={progress?.current ?? 0} /></div>}<div className="panel">{historyList(activity)}</div></>}
    {page === "settings" && <><div className="pageHeading"><div><h1>Settings</h1><p>App version and automatic update checks.</p></div></div><section className="panel settingsPanel"><div className="sectionHeading"><div><h2>Modpack Manager</h2><p>Installed version {appVersion}</p></div><span className="statusPill">{!appChecked || appChecking ? "Checking…" : appCheckFailed ? "Check unavailable" : appUpdate ? "Update available" : "Up to date"}</span></div>
     <p>App updates are checked every six hours. Your packs are checked every 30 minutes.</p>
     {appUpdate && <div className="appUpdateAvailable"><strong>Version {appUpdate.version} is available</strong><p>{appUpdate.body || "A new signed update is ready."}</p></div>}
     <div className="actions"><button className="secondary" disabled={appChecking || appInstalling} onClick={() => void checkAppUpdater(false)}>Check for app updates</button>{appUpdate && <button className="primary" disabled={busy} onClick={() => void installAppUpdate()}>Update app and restart</button>}</div>
     {appUpdateStatus && <p role="status">{appUpdateStatus}</p>}
    </section></>}
   </main>
  </div>
  {appUpdate && showUpdateToast && <aside className="updateToast" role="status"><div><strong>App update available</strong><span>Modpack Manager {appUpdate.version}</span></div><button className="textButton" onClick={() => { setPage("settings"); setShowUpdateToast(false); }}>View</button><button className="iconButton" aria-label="Dismiss update notification" onClick={() => setShowUpdateToast(false)}><Icon name="close" /></button></aside>}
  {(operation || appInstalling) && <div className="transferBar" role="status"><span className="transferIcon"><Icon name="download" /></span><div><strong>{operation?.title ?? "Updating Modpack Manager"}</strong><span>{appInstalling ? appUpdateStatus : progress?.message ?? "Preparing…"}</span></div>{operation && <progress max={Math.max(progress?.total ?? 1, 1)} value={progress?.current ?? 0} />}<button className="textButton" onClick={() => setPage(appInstalling ? "settings" : "activity")}>View {appInstalling ? "update" : "activity"} →</button></div>}
  <dialog ref={dialog} className="addDialog" aria-labelledby="wizard-title" onCancel={(event) => { if (validating) event.preventDefault(); else setWizardOpen(false); }}>
   <div className="dialogHeading"><div><small>ADD PACK</small><h2 id="wizard-title">{step === "choose" ? "Make room for your next adventure" : step === "existing" ? (replaceRoot ? "Change installation folder" : "Use an existing installation") : step === "location" ? "Where should TFG live?" : step === "review" ? "Ready to install" : step === "installing" ? "Installing your pack" : "Your pack is ready"}</h2></div><button className="iconButton" disabled={validating} aria-label="Close Add pack" onClick={() => setWizardOpen(false)}><Icon name="close" /></button></div>
   {wizardError && <div className="notice error" role="alert">{wizardError}</div>}
   {step === "choose" && <div className="wizardChoices"><button autoFocus onClick={() => setStep("location")}><span className="choiceIcon"><Icon name="download" /></span><strong>Install new</strong><span>Get the complete TFG NsTut pack. No existing pack needed.</span></button><button onClick={() => setStep("existing")}><span className="choiceIcon"><Icon name="folder" /></span><strong>Use existing installation</strong><span>Connect a pack from Prism, CurseForge, or another launcher.</span></button></div>}
   {(step === "existing" || step === "location") && <><p>{step === "existing" ? "Choose the game folder containing mods and config inside your TFG instance." : "Choose a new or empty folder. We’ll prepare the complete pack and a Prism Launcher import ZIP."}</p><label>{step === "existing" ? "Game folder" : "New installation folder"}<div className="pathRow"><input autoFocus aria-label={step === "existing" ? "Game folder" : "New installation folder"} value={folder} disabled={validating} spellCheck={false} placeholder={step === "existing" ? "Your instance’s .minecraft or minecraft folder" : "New or empty folder"} onChange={(event) => { setFolder(event.target.value); setWizardError(""); }} /><button className="secondary" disabled={validating} onClick={() => void browseFolder()}>Browse</button></div></label><div className="dialogActions"><button className="textButton" disabled={validating} onClick={() => { setStep("choose"); setWizardError(""); setReplaceRoot(undefined); setFolder(""); }}>Back</button><button className="primary" disabled={validating || busy || !folder.trim()} onClick={() => void validateFolder()}>{validating ? "Checking folder…" : step === "existing" ? "Add installation" : "Continue"}</button></div></>}
   {step === "review" && installPlan && <><div className="installSummary"><span className="choiceIcon"><Icon name="packs" /></span><div><strong>TerraFirmaGreg · NsTut</strong><p>{installPlan.version} · Minecraft {installPlan.minecraftVersion}</p></div></div><dl><dt>Install location</dt><dd><code>{installPlan.destination}</code></dd><dt>Play with</dt><dd>Prism Launcher · Forge {installPlan.forgeVersion}</dd></dl><p>All pack files will be downloaded and verified. Allow space for the pack and its import ZIP.</p><div className="dialogActions"><button className="textButton" disabled={busy} onClick={() => setStep("location")}>Back</button><button className="primary" disabled={busy} onClick={() => void freshInstall()}>Install pack</button></div></>}
   {step === "installing" && <><p>{progress?.message ?? "Preparing your download…"}</p><progress max={Math.max(progress?.total ?? 1, 1)} value={progress?.current ?? 0} /><p className="muted">You can keep using the Manager while this runs.</p><button className="secondary" onClick={() => setWizardOpen(false)}>Continue in background</button></>}
   {step === "finish" && installed && <><p>Import your pack in Prism Launcher: <strong>Add Instance → Import</strong>, then select:</p><code className="archivePath">{installed.archivePath}</code><p>After importing, connect the playable instance’s game folder here for future updates. You can also launch the prepared instance directly if you installed it in Prism’s instances folder.</p><div className="dialogActions"><button className="secondary" onClick={() => void openFolder(installed.instancePath)}>Open install folder</button><button className="primary" onClick={() => { setWizardOpen(false); selectPack(installed.minecraftRoot, true); }}>View pack</button></div></>}
  </dialog>
 </div>;
}
