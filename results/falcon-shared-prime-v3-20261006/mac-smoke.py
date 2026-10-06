import json,sys,pathlib
sys.path.insert(0,str(pathlib.Path("scripts").resolve()))
from falcon_v2_campaign import run_one,save
base=pathlib.Path(".tmp/falcon-v3")
manifest=json.loads((base/"manifest.json").read_text())
threads=int(sys.argv[1]); out=base/"smoke"; out.mkdir()
records=[]; compatibility=[]
for sec in [100,128]:
 r=run_one(out,manifest["binaries"]["v3"],"native",(sec,1,threads,42),0,1,180)
 reference=pathlib.Path(f"results/falcon-shared-prime-v2-20261006/native-compatibility-{sec}-v1.jsonl")
 before=next(x for x in map(json.loads,reference.read_text().splitlines()) if x.get("event")=="trial")
 after=next(x for x in map(json.loads,(out/f"s{sec}-b1-t{threads}-seed42-native.jsonl").read_text().splitlines()) if x.get("event")=="trial")
 keys=["proof_debug_digest","proof_payload_bytes","roots","input_digest"]
 assert all(before[k]==after[k] for k in keys),(sec,"native compatibility mismatch")
 compatibility.append(dict(security=sec,matched_keys=keys,reference=str(reference)))
 records.append(r)
for sec,batch in [(100,1),(100,3),(100,32),(100,1024),(128,1),(128,3),(128,32)]:
 records.append(run_one(out,manifest["binaries"]["v3"],"v3",(sec,batch,threads,42),0,1,180))
save(out/"summary.json",dict(verified_proofs=sum(r["verified_proofs"] for r in records),native_compatibility=compatibility,cases=[dict(label=r["label"],case=r["case"],payload=r["payload_bytes"],proof_digest=r["proof_digest"]) for r in records]))
print("All smoke proofs and native compatibility checks passed",flush=True)
