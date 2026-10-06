# Falcon one-source proof payload

Both batch-1024, seed-42 SharedPrime proofs verified. These are actual stored
payload counts, excluding the public statement and Falcon transport framing.
The baseline includes the upper-column u32 encoding from `51405193`.

| Security | Baseline payload | One-source payload | Reduction |
| --- | ---: | ---: | ---: |
| 100 bits | 892,466 B | **700,466 B** | 192,000 B (21.51%) |
| 128 bits | 1,098,266 B | **867,546 B** | 230,720 B (21.01%) |

| One-source component | 100 bits | 128 bits |
| --- | ---: | ---: |
| Integer column sums | 262,144 B | 327,680 B |
| Initial source authentication | 93,536 B | 116,960 B |
| Recursive and final authentication | 144,736 B | 175,968 B |
| Initial, recursive and final opened field values | 136,832 B | 175,360 B |
| Remaining proof messages and component framing | 63,218 B | 71,578 B |

The initial source has one multiproof over complete sixteen-lane rows.
Recursive PCS commitments still need their own authentication. Integer
column sums remain the largest individual component, at 37.4% / 37.8%.

Actual query locations differ from the old transcript, explaining the small
difference from the analytical 701,362 / 865,946 byte projections. Initial
query counts and all security parameters are unchanged. The reported
composition bounds remain 101.835 / 129.331 bits under the existing
computational grinding model, with hash collision security accounted for
separately.

The [raw records](seed42-b1024.jsonl) contain disjoint component counts, input
digests and complete proof Debug digests. Their component sums equal the
whole-proof payload. Runtime qualification is a separate experiment; these
size measurements do not establish the 2% performance gate.
