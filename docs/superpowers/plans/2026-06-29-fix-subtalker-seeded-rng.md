# Subtalker Seeded RNG Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make an explicit synthesis seed control both codebook-0 sampling and the code predictor's codebook 1–15 sampling.

**Architecture:** Keep the sampling configuration introduced by PR #21. Extend `CodePredictor::generate_codes` with a borrowed `SamplingKey`, route every generation mode's existing key into it, and centralize subtalker token sampling in a small helper that can be tested with deterministic logits.

**Tech Stack:** Rust, MLX, Cargo unit tests.

---

### Task 1: Cover and fix seeded subtalker sampling

**Files:**
- Modify: `node-hub/dora-qwen3-tts-mlx/patches/qwen3-tts-mlx/src/talker.rs`
- Modify: `node-hub/dora-qwen3-tts-mlx/patches/qwen3-tts-mlx/src/generate.rs`

- [ ] **Step 1: Write the failing deterministic sampling test**

Add a private `sample_subtalker_token` contract test in `talker.rs`:

```rust
#[test]
fn subtalker_sampling_repeats_with_the_same_seed() {
    let logits = Array::from_slice(&[0.0f32, 0.25, 0.5, 0.75], &[4]);

    let sample_sequence = |seed| {
        let mut key = SamplingKey::new(seed).unwrap();
        (0..16)
            .map(|_| {
                sample_subtalker_token(&logits, 1.0, 0, 1.0, Some(&mut key)).unwrap()
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(sample_sequence(4242), sample_sequence(4242));
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
cargo test -p qwen3-tts-mlx subtalker_sampling_repeats_with_the_same_seed -- --nocapture
```

Expected: compilation fails because `sample_subtalker_token` does not exist.

- [ ] **Step 3: Implement the minimal seeded path**

In `talker.rs`, import `SamplingKey`, add `sample_subtalker_token`, add `rng_key: Option<&mut SamplingKey>` to `generate_codes`, and reborrow that key for every codebook:

```rust
fn sample_subtalker_token(
    logits: &Array,
    temperature: f32,
    top_k: i32,
    top_p: f32,
    rng_key: Option<&mut SamplingKey>,
) -> Result<u32> {
    Ok(sample_logits(
        logits,
        temperature,
        top_k,
        top_p,
        1.0,
        &[],
        rng_key,
    )?)
}
```

In `generate.rs`, pass each mode's existing `rng_key.as_mut()` to every `generate_codes` call, including `GenerationState`.

- [ ] **Step 4: Verify GREEN and regressions**

Run:

```bash
cargo test -p qwen3-tts-mlx subtalker_sampling_repeats_with_the_same_seed -- --nocapture
cargo test -p qwen3-tts-mlx --lib -- --nocapture
cargo check -p dora-qwen3-tts-mlx
git diff --check
```

Expected: all commands exit successfully.

- [ ] **Step 5: Review the diff**

Confirm only the plan plus `generate.rs` and `talker.rs` changed, every generation mode passes the seeded key, and the original dirty `codex/tts-emotion-settings` worktree remains untouched.
