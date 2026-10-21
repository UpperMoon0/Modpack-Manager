// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { PACKS_KEY, ACTIVITY_KEY } from "./library";
import { ROOT_KEY } from "./channel";
import type { PatchProgress, TfgPlanResponse } from "./types";
const mocks = vi.hoisted(() => ({ plan: vi.fn(), state: vi.fn(), apply: vi.fn(), installPlan: vi.fn(), install: vi.fn(), openFolder: vi.fn(), check: vi.fn(), open: vi.fn(), progress: null as null | ((event: { payload: PatchProgress }) => void) }));
vi.mock("./api", () => ({tfgApi: mocks}));
vi.mock("@tauri-apps/api/app", () => ({getVersion: async () => "1.0.9"}));
vi.mock("@tauri-apps/api/event", () => ({listen: async (_: string, callback: typeof mocks.progress) => {mocks.progress = callback; return () => {};}}));
vi.mock("@tauri-apps/plugin-dialog", () => ({open: mocks.open}));
vi.mock("@tauri-apps/plugin-process", () => ({relaunch: vi.fn()}));
vi.mock("@tauri-apps/plugin-updater", () => ({check: mocks.check}));
function plan(root = "/packs/one/.minecraft", version = "0.13.12-nstut.1", changes = 0): TfgPlanResponse {
 return {patchVersion: version, mods: [{id:"speakers",name:"Simply Speakers",version:"0.9.1",fileName:"speakers.jar",source:"GitHub",targets:["client","server"],url:"https://example.test/mod.jar",sha256:"a".repeat(64)}],plan:{manifestId:"tfg",manifestName:"TFG",manifestVersion:version,target:"client",root,alreadyApplied:false,items:changes ? [{kind:"install",path:"mods/test.jar",detail:"Verified mod"}] : [],warnings:[]}};
}
function state(version = "0.13.12-nstut.1") {return {manifestId:"tfg",manifestVersion:version,target:"client",manifestSource:"tfg://",appliedAt:"2026-10-14T13:00:00Z"};}
function deferred<T>() {let resolve!: (value:T)=>void;const promise=new Promise<T>(r=>{resolve=r;});return {promise,resolve};}
beforeEach(() => {
 vi.clearAllMocks(); localStorage.clear(); mocks.progress=null;
 mocks.plan.mockImplementation(async root => plan(root)); mocks.state.mockResolvedValue(state()); mocks.check.mockResolvedValue(null);
 Object.defineProperty(HTMLDialogElement.prototype,"showModal",{configurable:true,value:function(){this.setAttribute("open","");}});
 Object.defineProperty(HTMLDialogElement.prototype,"close",{configurable:true,value:function(){this.removeAttribute("open");}});
});
afterEach(cleanup);
describe("Manager navigation", () => {
 it("updates the clicked card directly, locks other actions and refreshes that installation", async () => {
  const one="/packs/one/.minecraft",two="/packs/two/.minecraft",version="0.13.15-nstut.4";
  const pending=deferred<{state:ReturnType<typeof state>;backupPath:string;changedPaths:number}>();
  localStorage.setItem(ROOT_KEY,one);localStorage.setItem(PACKS_KEY,JSON.stringify([{root:one},{root:two}]));
  mocks.plan.mockImplementation(async root=>plan(root,version,1));mocks.apply.mockReturnValue(pending.promise);
  render(<App />);await screen.findAllByRole("button",{name:"Update pack"});
  const cards=screen.getAllByRole("article"),button=within(cards[1]).getByRole("button",{name:"Update pack"});
  expect(within(cards[1]).getByText(`Update to ${version}. Close Minecraft first.`)).toBeTruthy();
  fireEvent.click(button);fireEvent.click(button);
  expect(mocks.apply).toHaveBeenCalledTimes(1);expect(mocks.apply).toHaveBeenCalledWith(two,version);
  expect((within(cards[0]).getByRole("button",{name:"Update pack"}) as HTMLButtonElement).disabled).toBe(true);
  await act(async()=>mocks.progress?.({payload:{phase:"downloading",message:"Downloading mods",current:1,total:3}}));
  expect(screen.getByText("Downloading mods")).toBeTruthy();
  expect(screen.getByRole("heading",{name:"My packs"})).toBeTruthy();
  mocks.plan.mockImplementation(async root=>plan(root,version,root===two?0:1));
  mocks.state.mockResolvedValue(state(version));
  await act(async()=>pending.resolve({state:state(version),backupPath:"/backup/two",changedPaths:3}));
  await waitFor(()=>expect(within(screen.getAllByRole("article")[1]).getByText("Up to date")).toBeTruthy());
  expect(within(screen.getAllByRole("article")[1]).getByRole("button",{name:"Check for updates"})).toBeTruthy();
  expect(JSON.parse(localStorage.getItem(ACTIVITY_KEY)!)[0].root).toBe(two);
 });
 it("offers repair on the card for drift in an already installed version", async () => {
  const root="/packs/one/.minecraft",version="0.13.15-nstut.4";
  localStorage.setItem(ROOT_KEY,root);
  mocks.plan.mockImplementation(async()=>{const result=plan(root,version,1);result.plan.alreadyApplied=true;return result;});
  mocks.apply.mockReturnValue(deferred().promise);
  render(<App />);fireEvent.click(await screen.findByRole("button",{name:"Repair pack"}));
  expect(mocks.apply).toHaveBeenCalledWith(root,version);
 });
 it("disables the card update while its checked plan is being refreshed", async () => {
  vi.useFakeTimers();
  try {
   const pending=deferred<TfgPlanResponse>();localStorage.setItem(ROOT_KEY,"/packs/one/.minecraft");
   mocks.plan.mockResolvedValueOnce(plan(undefined,"0.13.15-nstut.4",1)).mockReturnValueOnce(pending.promise);
   render(<App />);await act(async()=>{await Promise.resolve();});
   await act(async()=>{await vi.advanceTimersByTimeAsync(30*60*1000);});
   const button=screen.getByRole("button",{name:"Update pack"}) as HTMLButtonElement;
   expect(button.disabled).toBe(true);fireEvent.click(button);expect(mocks.apply).not.toHaveBeenCalled();
   await act(async()=>pending.resolve(plan()));
   expect(screen.queryByRole("button",{name:"Update pack"})).toBeNull();
   expect(screen.getByRole("button",{name:"Check for updates"})).toBeTruthy();
  } finally {vi.useRealTimers();}
 });
 it("migrates the old folder, hides setup on the main page and separates searchable mods and settings", async () => {
  localStorage.setItem(ROOT_KEY,"/packs/one/.minecraft"); render(<App />);
  await screen.findByText("Up to date");
  expect(screen.queryByLabelText("New installation folder")).toBeNull();expect(screen.queryByText("Simply Speakers")).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"Details →"}));
  fireEvent.click(screen.getByRole("tab",{name:"Mods"}));await screen.findByText("Simply Speakers");
  fireEvent.change(screen.getByRole("textbox",{name:"Search mods"}),{target:{value:"not-a-mod"}});expect(screen.queryByText("Simply Speakers")).toBeNull();expect(screen.getByText("No mods match your search.")).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"Settings"}));await screen.findByRole("heading",{name:"Modpack Manager"});
  expect(screen.queryByRole("heading",{name:"TerraFirmaGreg"})).toBeNull();
  expect(JSON.parse(localStorage.getItem(PACKS_KEY)!)[0].root).toBe("/packs/one/.minecraft");
 });
 it("keeps the transfer visible across pages and stores completion history for its actual installation", async () => {
  const pending=deferred<{state:ReturnType<typeof state>;backupPath:string;changedPaths:number}>();
  localStorage.setItem(ROOT_KEY,"/packs/one/.minecraft");mocks.plan.mockImplementation(async root=>plan(root,"0.13.12-nstut.2",1));mocks.apply.mockReturnValue(pending.promise);
  render(<App />);await screen.findByText("Update available");fireEvent.click(screen.getByRole("button",{name:"Details →"}));fireEvent.click(screen.getByRole("button",{name:"Update pack"}));
  expect(mocks.apply).toHaveBeenCalledWith("/packs/one/.minecraft","0.13.12-nstut.2");
  await act(async()=>mocks.progress?.({payload:{phase:"downloading",message:"Downloading mods",current:3,total:10}}));
  fireEvent.click(screen.getByRole("button",{name:"Settings"}));expect(screen.getByText("Downloading mods")).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"Activity"}));expect(screen.getAllByText("Downloading mods").length).toBe(2);
  mocks.plan.mockImplementation(async root=>plan(root,"0.13.12-nstut.2"));
  await act(async()=>pending.resolve({state:state("0.13.12-nstut.2"),backupPath:"/backup/one",changedPaths:3}));
  await screen.findByText("Updated to 0.13.12-nstut.2");
  const entries=JSON.parse(localStorage.getItem(ACTIVITY_KEY)!);expect(entries[0].root).toBe("/packs/one/.minecraft");expect(entries[0].backupPath).toBe("/backup/one");
  cleanup();render(<App />);fireEvent.click(screen.getByRole("button",{name:"Activity"}));expect(screen.getByText("Updated to 0.13.12-nstut.2")).toBeTruthy();
 });
 it("keeps late checks scoped to their pack and does not reorder the library", async () => {
  const first=deferred<TfgPlanResponse>();const one="/packs/one/.minecraft",two="/packs/two/.minecraft";
  localStorage.setItem(PACKS_KEY,JSON.stringify([{root:one},{root:two}]));mocks.plan.mockImplementation(root=>root===one?first.promise:Promise.resolve(plan(root,"0.13.12-nstut.3",1)));
  render(<App />);await screen.findByText("Update available");fireEvent.click(within(screen.getAllByRole("article")[1]).getByRole("button",{name:"Details →"}));
  await act(async()=>first.resolve(plan(one,"0.13.12-nstut.1")));
  expect(screen.getByText("0.13.12-nstut.3")).toBeTruthy();fireEvent.click(screen.getByRole("button",{name:"Update pack"}));
  await waitFor(()=>expect(mocks.apply).toHaveBeenCalledWith(two,"0.13.12-nstut.3"));
  expect(JSON.parse(localStorage.getItem(PACKS_KEY)!).map((p:{root:string})=>p.root)).toEqual([one,two]);
 });
 it("ignores an older response when a newer check already finished", async()=>{
  vi.useFakeTimers();
  try {
   const old=deferred<TfgPlanResponse>();localStorage.setItem(ROOT_KEY,"/packs/one/.minecraft");mocks.plan.mockReturnValueOnce(old.promise).mockResolvedValue(plan(undefined,"0.13.12-nstut.4",1));
   render(<App />);
   await act(async()=>{await vi.advanceTimersByTimeAsync(30*60*1000);});
   fireEvent.click(screen.getByRole("button",{name:"Details →"}));
   expect(mocks.plan).toHaveBeenCalledTimes(2);
   await act(async()=>old.resolve(plan()));
   expect(screen.getByText("0.13.12-nstut.4")).toBeTruthy();
  } finally {vi.useRealTimers();}
 });
 it("does not restore a replaced folder when its older check completes", async () => {
  const old = deferred<TfgPlanResponse>();
  const previous = "/packs/old/.minecraft", replacement = "/packs/new/.minecraft";
  localStorage.setItem(ROOT_KEY, previous);
  mocks.plan.mockImplementation(root => root === previous ? old.promise : Promise.resolve(plan(root)));
  render(<App />);
  fireEvent.click(screen.getByText("Change folder"));
  fireEvent.change(screen.getByRole("textbox", { name: "Game folder" }), { target: { value: replacement } });
  fireEvent.click(screen.getByRole("button", { name: "Add installation" }));
  await waitFor(() => expect(localStorage.getItem(ROOT_KEY)).toBe(replacement));
  await act(async () => old.resolve(plan(previous)));
  expect(JSON.parse(localStorage.getItem(PACKS_KEY)!).map((pack: { root: string }) => pack.root)).toEqual([replacement]);
  fireEvent.click(screen.getAllByRole("button", { name: "My packs" })[0]);
  expect(screen.getAllByRole("article")).toHaveLength(1);
 });
 it("validates a new install in a wizard and passes the reviewed version to installation",async()=>{
  mocks.installPlan.mockResolvedValue({version:"0.13.12-nstut.1",destination:"/new",artifactCount:304,minecraftVersion:"1.20.1",forgeVersion:"47.4.13"});mocks.install.mockResolvedValue({version:"0.13.12-nstut.1",minecraftRoot:"/new/.minecraft",instancePath:"/new",archivePath:"/new/TFG.zip"});
  render(<App />);fireEvent.click(screen.getByRole("button",{name:"Add your first pack"}));fireEvent.click(screen.getByRole("button",{name:/Install new/}));
  fireEvent.change(screen.getByRole("textbox",{name:"New installation folder"}),{target:{value:"/new"}});fireEvent.click(screen.getByRole("button",{name:"Continue"}));await screen.findByText("Ready to install");
  fireEvent.click(screen.getByRole("button",{name:"Install pack"}));await screen.findByText("Your pack is ready");expect(mocks.install).toHaveBeenCalledWith("/new","0.13.12-nstut.1");expect(screen.getByText("/new/TFG.zip")).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"View pack"}));expect(localStorage.getItem(ROOT_KEY)).toBe("/new/.minecraft");
 });
 it("does not register an invalid existing folder",async()=>{
  mocks.plan.mockRejectedValue(new Error("Not a compatible pack"));render(<App />);fireEvent.click(screen.getByRole("button",{name:"Add your first pack"}));fireEvent.click(screen.getByRole("button",{name:/Use existing installation/}));
  fireEvent.change(screen.getByRole("textbox",{name:"Game folder"}),{target:{value:"/wrong"}});fireEvent.click(screen.getByRole("button",{name:"Add installation"}));await screen.findByText("Not a compatible pack");expect(JSON.parse(localStorage.getItem(PACKS_KEY)!)).toEqual([]);expect(localStorage.getItem(ROOT_KEY)).toBeNull();
 });
});
