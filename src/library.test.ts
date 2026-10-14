import { describe, expect, it } from "vitest";
import { readLibrary, rememberPack, rootKey } from "./library";
import { ROOT_KEY } from "./channel";
describe("pack library",()=>{
 it("preserves Linux case while identifying Windows spellings of the same root",()=>{expect(rootKey("D:\\Packs\\TFG\\.minecraft\\")).toBe(rootKey("d:/packs/tfg/.minecraft"));expect(rootKey("/packs/TFG")).not.toBe(rootKey("/packs/tfg"));});
 it("recovers the legacy root from corrupt library metadata",()=>{const storage={getItem:(key:string)=>key===ROOT_KEY?"/old/.minecraft":"broken"};expect(readLibrary(storage)).toEqual([{root:"/old/.minecraft",version:null,appliedAt:null}]);});
 it("clears cached installed metadata when the backend confirms no patch state", () => {
  const packs = [{ root: "/one", version: "old", appliedAt: "2026-10-01T00:00:00Z" }];
  expect(rememberPack(packs, "/one", null)).toEqual([{ root: "/one", version: null, appliedAt: null }]);
 });
 it("keeps pack order stable while updating metadata",()=>{const packs=[{root:"/one",version:null,appliedAt:null},{root:"/two",version:null,appliedAt:null}];expect(rememberPack(packs,"/one",null).map(p=>p.root)).toEqual(["/one","/two"]);});
});
