# Enrich the `verified` event with `model_hash` and `output` (Closes #63)

## Summary

The `verified` event currently publishes only the ledger sequence number
(`record.verified_at`). An off-chain indexer watching the contract to learn
"which model produced which output" cannot get that from the event — it has to
call back into `get_result`, which returns only the single most recent record
and races against the next verification.

This changes the `verified` event so the model hash and output are carried in
the event itself, making it possible to build an indexer directly off the event
stream.

> This is a **breaking change** to the event payload shape (the `data` field
> changes from a bare `u32` to a `(u32, Bytes)` tuple), so the contract
> interface `VERSION` is bumped `2 -> 3`.

## What changed

### `crates/zkml-verifier/src/lib.rs`
- Extracted event emission into `emit_verified_event(env, record)` and call it
  from the successful path of `verify_inference`.
- The event is now published as:
  ```rust
  env.events().publish(
      (symbol_short!("verified"), record.model_hash.clone()),
      (record.verified_at, record.output.clone()),
  );
  ```
  - `model_hash` is published as a **topic** (topic[1]) so indexers can
    subscribe/filter per model once more than one model is registered across
    deployments.
  - `verified_at` (ledger sequence) and `output` are published as **data**
    (data[0] and data[1]). `output` is kept out of the topics because it is
    variable-length and not a good filter key.
- Bumped `VERSION` `2 -> 3` to signal the breaking event-shape change.

### `docs/verifier-interface.md`
- Added an **Events** section documenting the `verified` event shape
  (topics + data), the rationale for `model_hash` as a topic, and how consumers
  should filter.

### Tests
- Added `test_verified_event::verify_emits_verified_event_with_model_hash_and_output`
  which emits the event with a known `InferenceRecord` and asserts the emitted
  event's topics contain `verified` + the `model_hash`, and the data contains
  `verified_at` + `output`.

All existing verifier tests still pass (13/13).

## Acceptance criteria
- [x] The `verified` event includes `model_hash` and `output`.
- [x] The event shape is documented in `docs/verifier-interface.md`.
- [x] A test asserts the event payload contents.

## Notes for reviewers
- The `output` topic limit: `model_hash` is exactly 32 bytes, which is within
  Soroban's 32-byte topic limit for `Bytes`, so it is safe as a topic.
- Indexers should filter `topic[0] == "verified"` and may additionally filter
  `topic[1] == <model_hash>` to track a single model.

---

Closes #63
