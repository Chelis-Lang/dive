# 0163: Compress independent disk-frontier records

- **Status:** accepted
- **Date:** 2026-11-20

## Context

BPConProof’s independent frontier wrote 49.69 GB by its hundred-millionth discovery. We sampled 64 evenly spaced records from each active breadth-first level of a longer run. LZ4 reduced the 128 payloads from 4,014,450 to 849,886 bytes and every decompressed payload matched its input.

## Decision

Use the safe block encoder and decoder from the MIT-licensed `lz4_flex` crate in `dive-explore`. Each disk record is an independently readable chunk. Keep the original encoding when compression would increase its size. The queue checksum covers the stored bytes. Check the declared decoded size against the existing 64 MiB limit before reserving memory, then validate the actual decoded length and the owned DAG.

## Consequences

Scratch consumption now depends on repetition within each group of up to 64 rows. Encoding and decoding use additional bounded workspace and CPU. The [large-model guide](../developer/large-models.md#high-fanout-parallel-safety) records the measured end-to-end tradeoff. The wire format used by distributed workers and the handle-based frontier keep their existing encodings.
