# Full Falcon-1024 proof sizes, batch 1,024

Both proofs include SHAKE-256 and HashToPoint and verified successfully on the same seed-42 input corpus. The implementation and binary match the full thread sweep. The 100-bit size is from one fresh proof at the auto-selected K9 profile; the 128-bit K11 size was identical across all 30 sweep trials.

| Security target | Ring extension | Raw proof payload | Payload including source root | KiB including root |
|---:|---:|---:|---:|---:|
| 100 bits | 9 | 652,622 B | 652,654 B | 637.36 |
| 128 bits | 11 | 814,554 B | 814,586 B | 795.49 |

The root-inclusive figures add the 32-byte source commitment root stored in the public statement. All other public inputs and outer transport framing are excluded. Query-dependent authentication sizes can vary with the input corpus. These are measured sizes for these inputs, not universal size bounds.
