# CustomVoice Instruct Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an expandable, user-editable CustomVoice instruct editor with emotion presets, deterministic history restoration, supported-path visibility, and a stronger Angry preset.

**Architecture:** Keep model-facing instruct behavior in `tts_emotion.rs` as a small, testable state model. `screen.rs` owns Makepad widget synchronization and reads the state model's final text when creating a generation snapshot. Existing Dora and Qwen MLX propagation remain unchanged except for regression verification because the current worktree already carries the official CustomVoice ChatML and prefill integration.

**Tech Stack:** Rust, Makepad widgets/live design, Serde JSON history, Dora, Qwen3-TTS MLX, Cargo tests, macOS MLX/Metal listening tests.

**Working agreement:** Do not stage, commit, or push this CustomVoice branch. Each task ends with a local diff/test checkpoint instead of a commit.

---

## File Map

- Modify `apps/moxin-voice/src/tts_emotion.rs`
  - Own preset definitions, localized instruct values, editor state transitions, request eligibility, length validation, and history classification.
- Modify `apps/moxin-voice/src/screen.rs`
  - Add the compact expandable UI, handle widget events, synchronize state to widgets, validate generation, and restore history.
- Keep `apps/moxin-voice/src/tts_history.rs`
  - Preserve the already-added optional `emotion_id`, `emotion_label`, and `instruct` fields and their backward-compatible tests.
- Verify `apps/moxin-voice/src/lib.rs`
  - Ensure `tts_emotion` remains registered as an application module.
- Verify `node-hub/dora-qwen3-tts-mlx/src/main.rs`
  - Ensure the final instruct string is parsed and passed to `SynthesizeOptions`.
- Verify `node-hub/dora-qwen3-tts-mlx/patches/qwen3-tts-mlx/src/*.rs`
  - Retain official tokenizer special-token loading, CustomVoice ChatML prefill, and generation sampling fixes already present in the worktree.

## Task 1: Make Instruct State a Testable Domain Model

**Files:**
- Modify: `apps/moxin-voice/src/tts_emotion.rs`

- [ ] **Step 1: Add failing tests for preset values and state transitions**

Add tests that exercise the public state API before implementing it:

```rust
#[test]
fn angry_uses_approved_strong_instruct_in_both_languages() {
    let angry = emotion_preset("angry").unwrap();
    assert_eq!(
        angry.instruct(false),
        Some("用极其愤怒、严厉斥责、情绪爆发的语气说，语调强烈，咬字有力")
    );
    assert_eq!(
        angry.instruct(true),
        Some(
            "Use an extremely angry, stern, emotionally explosive tone, with forceful intonation and articulation."
        )
    );
}

#[test]
fn preset_edit_and_clear_follow_custom_state_rules() {
    let mut state = TtsInstructState::default();
    state.select_preset("happy", false);
    assert_eq!(state.selection(), TtsInstructSelection::Preset("happy"));
    assert_eq!(state.text(), "用开心、轻快的语气说");

    state.edit("请带着克制但明显的喜悦说".to_string());
    assert_eq!(state.selection(), TtsInstructSelection::Custom);

    state.edit(String::new());
    assert_eq!(state.selection(), TtsInstructSelection::Neutral);
    assert_eq!(state.effective_instruct(), None);
}

#[test]
fn whitespace_only_text_remains_custom_and_is_not_trimmed() {
    let mut state = TtsInstructState::default();
    state.edit("  \n".to_string());
    assert_eq!(state.selection(), TtsInstructSelection::Custom);
    assert_eq!(state.effective_instruct(), Some("  \n"));
}

#[test]
fn history_matches_any_localized_preset_variant() {
    assert_eq!(
        TtsInstructState::from_history(Some("Say it in a happy and lively tone")),
        TtsInstructState::from_preset("happy", true)
    );
    assert_eq!(
        TtsInstructState::from_history(Some("用开心、轻快的语气说")).selection(),
        TtsInstructSelection::Preset("happy")
    );
}
```

- [ ] **Step 2: Run the tests and confirm they fail for missing state APIs**

Run:

```bash
cargo test -p moxin-voice --lib tts_emotion::tests -- --nocapture
```

Expected: compilation fails because `TtsInstructState` and `TtsInstructSelection` are not defined, and the old Angry values do not match.

- [ ] **Step 3: Implement the minimal state model and stronger Angry preset**

Add:

```rust
pub const CUSTOM_EMOTION_ID: &str = "custom";
pub const TTS_INSTRUCT_MAX_CHARS: usize = 200;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TtsInstructSelection {
    #[default]
    Neutral,
    Preset(&'static str),
    Custom,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TtsInstructState {
    selection: TtsInstructSelection,
    text: String,
    editor_expanded: bool,
}
```

Implement these methods with exact-empty semantics:

```rust
impl TtsInstructState {
    pub fn from_preset(id: &str, english: bool) -> Self;
    pub fn from_history(instruct: Option<&str>) -> Self;
    pub fn select_preset(&mut self, id: &str, english: bool);
    pub fn edit(&mut self, text: String);
    pub fn collapse(&mut self);
    pub fn toggle_editor(&mut self);
    pub fn selection(&self) -> TtsInstructSelection;
    pub fn selection_id(&self) -> &'static str;
    pub fn text(&self) -> &str;
    pub fn editor_expanded(&self) -> bool;
    pub fn effective_instruct(&self) -> Option<&str>;
    pub fn char_count(&self) -> usize;
    pub fn is_over_limit(&self) -> bool;
}
```

`from_history` must compare against every Chinese and English preset instruct and must return Custom for unmatched non-empty text. Replace the Angry preset values with the approved strong Chinese and English instructions.

- [ ] **Step 4: Run the focused tests**

Run:

```bash
cargo test -p moxin-voice --lib tts_emotion::tests -- --nocapture
```

Expected: all `tts_emotion` tests pass.

- [ ] **Step 5: Check the local diff without staging**

Run:

```bash
git diff --check -- apps/moxin-voice/src/tts_emotion.rs
git status --short
```

Expected: no whitespace errors; `tts_emotion.rs` remains unstaged.

## Task 2: Add Supported-Path Eligibility and Request Filtering

**Files:**
- Modify: `apps/moxin-voice/src/tts_emotion.rs`
- Modify: `apps/moxin-voice/src/screen.rs`

- [ ] **Step 1: Add failing eligibility tests**

Add:

```rust
#[test]
fn instruct_is_supported_only_for_qwen_builtin_voices() {
    assert!(supports_custom_voice_instruct("qwen3_tts_mlx", true));
    assert!(!supports_custom_voice_instruct("qwen3_tts_mlx", false));
    assert!(!supports_custom_voice_instruct("primespeech", true));
}

#[test]
fn unsupported_request_omits_preserved_instruct() {
    let state = TtsInstructState::from_preset("happy", false);
    assert_eq!(
        instruct_for_request(&state, "qwen3_tts_mlx", true),
        Some("用开心、轻快的语气说")
    );
    assert_eq!(instruct_for_request(&state, "primespeech", true), None);
    assert_eq!(instruct_for_request(&state, "qwen3_tts_mlx", false), None);
}
```

- [ ] **Step 2: Run the focused tests and confirm failure**

Run:

```bash
cargo test -p moxin-voice --lib tts_emotion::tests -- --nocapture
```

Expected: compilation fails for missing `supports_custom_voice_instruct` and `instruct_for_request`.

- [ ] **Step 3: Implement eligibility helpers**

Add pure helpers:

```rust
pub fn supports_custom_voice_instruct(backend: &str, is_builtin_voice: bool) -> bool {
    backend == "qwen3_tts_mlx" && is_builtin_voice
}

pub fn instruct_for_request<'a>(
    state: &'a TtsInstructState,
    backend: &str,
    is_builtin_voice: bool,
) -> Option<&'a str> {
    supports_custom_voice_instruct(backend, is_builtin_voice)
        .then(|| state.effective_instruct())
        .flatten()
}
```

In `screen.rs`, add a `selected_voice_is_builtin()` helper that checks the selected `VoiceSource`. Add `tts_instruct_state: TtsInstructState` to `TTSScreen`, initialize it to Neutral, and remove direct dependence on preset-derived instruct during generation.

- [ ] **Step 4: Run focused tests and compile the application crate**

Run:

```bash
cargo test -p moxin-voice --lib tts_emotion::tests -- --nocapture
cargo check -p moxin-voice
```

Expected: focused tests and `cargo check` pass, except for any pre-existing workspace warning output.

## Task 3: Build the Compact Expandable Makepad UI

**Files:**
- Modify: `apps/moxin-voice/src/screen.rs`

- [ ] **Step 1: Add the instruct UI below the preset row**

Reshape `emotion_row` into a vertical `emotion_section` containing the existing preset row and:

```rust
instruct_summary_row = <View> {
    width: Fill, height: Fit
    flow: Right
    align: {y: 0.5}
    spacing: 8

    instruct_summary = <Label> {
        width: Fill, height: Fit
        draw_text: {
            text_style: <FONT_REGULAR>{ font_size: 11.0 }
        }
        text: ""
    }

    edit_instruct_btn = <Button> {
        width: Fit, height: 28
        text: "编辑指令"
    }
}

instruct_editor = <View> {
    width: Fill, height: Fit
    flow: Down
    spacing: 6
    visible: false

    instruct_input = <TextInput> {
        width: Fill, height: 72
        empty_text: "输入希望音色采用的语气、情绪或表达方式"
    }

    instruct_editor_footer = <View> {
        width: Fill, height: Fit
        flow: Right
        align: {x: 1.0, y: 0.5}
        spacing: 8

        instruct_char_count = <Label> { text: "" }
        collapse_instruct_btn = <Button> { text: "收起" }
    }
}
```

Use the application’s existing restrained colors, 7px-or-smaller control radius, dark-mode fields, and typography. Do not add a surrounding card inside `input_section`.

- [ ] **Step 2: Add a non-clickable Custom status chip**

Add `emotion_custom_status` alongside the emotion options, initially hidden. It must use the same visual active treatment as `VoiceFilterChip`, display `自定义`/`Custom`, and not participate in click handling.

- [ ] **Step 3: Add widget synchronization methods**

Implement:

```rust
fn update_tts_instruct_availability(&mut self, cx: &mut Cx);
fn update_tts_instruct_controls(&mut self, cx: &mut Cx);
fn tts_instruct_summary(text: &str, max_chars: usize) -> String;
```

`update_tts_instruct_availability` must:

- Show the entire section only for Qwen plus a built-in voice.
- Collapse the editor when the section becomes hidden.
- Preserve the text and selection in `tts_instruct_state`.

`update_tts_instruct_controls` must:

- Apply active styling to Neutral, preset, or Custom.
- Keep Edit available in Neutral.
- Show summary only for non-empty text.
- Show the editor only when expanded and supported.
- Show the remaining count at 180 or more characters, and show an over-limit count after 200.
- Localize labels without modifying the current instruct text.

- [ ] **Step 4: Wire refresh points**

Call the new synchronization methods after:

- Application initialization.
- Preset selection.
- Voice selection and `sync_selected_voice_ui`.
- Backend/model selection.
- Language changes.
- History reuse.
- Editor expansion/collapse.

- [ ] **Step 5: Compile the Makepad live design**

Run:

```bash
cargo check -p moxin-voice-shell
```

Expected: Makepad live design and Rust compile successfully.

- [ ] **Step 6: Check layout diff**

Run:

```bash
git diff --check -- apps/moxin-voice/src/screen.rs
```

Expected: no whitespace errors and no unrelated layout churn.

## Task 4: Wire Preset, Edit, Collapse, and Localization Events

**Files:**
- Modify: `apps/moxin-voice/src/screen.rs`

- [ ] **Step 1: Replace preset-only selection with state transitions**

Update `select_tts_emotion`:

```rust
fn select_tts_emotion(&mut self, cx: &mut Cx, emotion_id: &str) {
    self.tts_instruct_state
        .select_preset(emotion_id, self.is_english());
    self.update_tts_instruct_controls(cx);
}
```

Keep `tts_emotion_id` only if another existing consumer still needs it; otherwise derive the metadata ID from `tts_instruct_state.selection_id()`.

- [ ] **Step 2: Handle editor buttons**

Add event handling:

```rust
if edit_instruct_btn.clicked(&actions) {
    self.tts_instruct_state.toggle_editor();
    self.update_tts_instruct_controls(cx);
}

if collapse_instruct_btn.clicked(&actions) {
    self.tts_instruct_state.collapse();
    self.update_tts_instruct_controls(cx);
}
```

- [ ] **Step 3: Handle exact user input**

Read `instruct_input.changed(&actions)` and pass the string directly to:

```rust
self.tts_instruct_state.edit(changed_text);
```

Do not trim, truncate, translate, or rewrite it. Refresh the Custom/Neutral selection, summary, and character count after every change.

- [ ] **Step 4: Preserve text across application language changes**

The localization refresh may update:

- Emotion labels.
- Edit/Collapse labels.
- Placeholder.
- Character-count suffix.

It must not call `select_preset` or replace `tts_instruct_state.text()`. Add a focused pure-state test proving that reading labels in another language does not mutate the state.

- [ ] **Step 5: Run state tests and compile**

Run:

```bash
cargo test -p moxin-voice --lib tts_emotion::tests -- --nocapture
cargo check -p moxin-voice-shell
```

Expected: all new state tests pass and the shell compiles.

## Task 5: Use Final Editable Text for Generation and Validation

**Files:**
- Modify: `apps/moxin-voice/src/screen.rs`

- [ ] **Step 1: Add failing length-boundary tests**

Add state tests:

```rust
#[test]
fn instruct_limit_counts_unicode_scalars() {
    let mut state = TtsInstructState::default();
    state.edit("情".repeat(TTS_INSTRUCT_MAX_CHARS));
    assert_eq!(state.char_count(), 200);
    assert!(!state.is_over_limit());

    state.edit("情".repeat(TTS_INSTRUCT_MAX_CHARS + 1));
    assert_eq!(state.char_count(), 201);
    assert!(state.is_over_limit());
}
```

- [ ] **Step 2: Run the focused test and confirm failure if limit APIs are incomplete**

Run:

```bash
cargo test -p moxin-voice --lib instruct_limit_counts_unicode_scalars -- --nocapture
```

Expected: FAIL until `char_count` and `is_over_limit` implement the 200-scalar rule.

- [ ] **Step 3: Block generation above 200 characters**

At the start of `generate_speech`, before changing player/loading state:

```rust
if self.tts_instruct_state.is_over_limit() {
    self.show_toast(
        cx,
        self.tr(
            "情绪指令超过 200 字符限制，请缩短后重试",
            "Emotion instruction exceeds the 200 character limit",
        ),
    );
    return;
}
```

Do not truncate the user’s text.

- [ ] **Step 4: Snapshot and send the final editable instruct**

Replace preset regeneration with:

```rust
let is_builtin_voice = voice_info
    .as_ref()
    .map(|voice| voice.source == VoiceSource::Builtin)
    .unwrap_or(false);
let final_instruct = tts_emotion::instruct_for_request(
    &self.tts_instruct_state,
    &self.app_preferences.inference_backend,
    is_builtin_voice,
)
.map(str::to_string);
```

Set:

```rust
self.pending_generation_emotion_id =
    Some(self.tts_instruct_state.selection_id().to_string());
self.pending_generation_emotion_label =
    Some(self.localized_instruct_selection_label());
self.pending_generation_instruct = final_instruct.clone();
```

Use `final_instruct` in the existing JSON payload. Unsupported paths must serialize no effective instruct even if the hidden state retains text.

- [ ] **Step 5: Avoid logging full custom prompt contents at info level**

Keep an info log indicating the selection ID and character count. Remove or downgrade any log that prints the full user-authored instruct, because it may contain private text.

- [ ] **Step 6: Run focused and payload regression tests**

Run:

```bash
cargo test -p moxin-voice --lib tts_emotion::tests -- --nocapture
cargo test -p dora-qwen3-tts-mlx --bin qwen-tts-node -- --nocapture
cargo check -p moxin-voice-shell
```

Expected: state tests, Dora payload parsing tests, and shell compilation pass.

## Task 6: Restore Final Instruct Deterministically from History

**Files:**
- Modify: `apps/moxin-voice/src/screen.rs`
- Verify: `apps/moxin-voice/src/tts_history.rs`

- [ ] **Step 1: Add focused history-classification tests**

Add tests in `tts_emotion.rs`:

```rust
#[test]
fn history_restore_classifies_neutral_preset_and_custom() {
    assert_eq!(
        TtsInstructState::from_history(None).selection(),
        TtsInstructSelection::Neutral
    );
    assert_eq!(
        TtsInstructState::from_history(Some("")).selection(),
        TtsInstructSelection::Neutral
    );
    assert_eq!(
        TtsInstructState::from_history(Some("Say it in a sad and subdued tone")).selection(),
        TtsInstructSelection::Preset("sad")
    );
    assert_eq!(
        TtsInstructState::from_history(Some("Very restrained, but clearly hopeful.")).selection(),
        TtsInstructSelection::Custom
    );
}
```

- [ ] **Step 2: Run the focused test**

Run:

```bash
cargo test -p moxin-voice --lib history_restore_classifies_neutral_preset_and_custom -- --nocapture
```

Expected: PASS after Task 1’s history classifier is complete.

- [ ] **Step 3: Restore history from the saved final text**

In `reuse_history_entry`, replace `emotion_id`-only restoration with:

```rust
self.tts_instruct_state =
    TtsInstructState::from_history(entry.instruct.as_deref());
self.tts_instruct_state.collapse();
self.update_tts_instruct_availability(cx);
self.update_tts_instruct_controls(cx);
```

The saved `emotion_id` and `emotion_label` remain display metadata; final instruct text determines Neutral/Preset/Custom restoration.

- [ ] **Step 4: Verify backward compatibility**

Run:

```bash
cargo test -p moxin-voice --lib tts_history::tests -- --nocapture
cargo test -p moxin-voice --lib tts_emotion::tests -- --nocapture
```

Expected: legacy history without emotion/instruct fields still deserializes, and all classification tests pass.

## Task 7: Run Backend Regression Verification

**Files:**
- Verify: `node-hub/dora-qwen3-tts-mlx/src/main.rs`
- Verify: `node-hub/dora-qwen3-tts-mlx/patches/qwen3-tts-mlx/examples/synthesize.rs`
- Verify: `node-hub/dora-qwen3-tts-mlx/patches/qwen3-tts-mlx/src/config.rs`
- Verify: `node-hub/dora-qwen3-tts-mlx/patches/qwen3-tts-mlx/src/generate.rs`
- Verify: `node-hub/dora-qwen3-tts-mlx/patches/qwen3-tts-mlx/src/lib.rs`
- Verify: `node-hub/dora-qwen3-tts-mlx/patches/qwen3-tts-mlx/src/sampling.rs`
- Verify: `node-hub/dora-qwen3-tts-mlx/patches/qwen3-tts-mlx/src/talker.rs`

- [ ] **Step 1: Run Qwen library tests**

Run:

```bash
cargo test -p qwen3-tts-mlx --lib -- --nocapture
```

Expected: tokenizer added-token, official ChatML slicing, sampling, and CustomVoice tests pass.

- [ ] **Step 2: Run Dora node tests**

Run:

```bash
cargo test -p dora-qwen3-tts-mlx --bin qwen-tts-node -- --nocapture
```

Expected: instruct parsing and propagation tests pass.

- [ ] **Step 3: Run CLI routing regression**

Run:

```bash
cargo test -p qwen3-tts-mlx --example synthesize custom_voice_instruct_stays_on_custom_voice_route -- --nocapture
```

Expected: CustomVoice instruct remains on the CustomVoice route; VoiceDesign is not selected.

- [ ] **Step 4: Run application tests**

Run:

```bash
cargo test -p moxin-voice --lib -- --nocapture
```

Expected: all feature tests pass. If `voice_persistence::tests::test_voice_id_with_chinese` still fails, verify it also fails from the clean baseline and record it as pre-existing.

## Task 8: Visual, Interaction, and Listening Verification

**Files:**
- Verify: `apps/moxin-voice/src/screen.rs`
- Output audio: `/private/tmp/moxin-customvoice-clean-listen/`

- [ ] **Step 1: Start the application without committing**

Run with the required local process/Metal permissions:

```bash
cargo run -p moxin-voice-shell
```

Expected: the app starts and the Qwen dataflow becomes ready.

- [ ] **Step 2: Verify supported-path UI**

With Qwen plus Vivian or another built-in Qwen speaker:

- Presets and Edit instruction are visible.
- Neutral can open an empty editor.
- Clicking Happy fills the localized Happy instruct.
- Editing one character selects Custom.
- Clearing all characters selects Neutral.
- Whitespace-only text remains Custom.
- Collapse preserves text and shows the one-line summary.
- Custom appears as status only and is not clickable.
- No text or controls overlap at normal and narrow window widths.

- [ ] **Step 3: Verify hidden unsupported paths**

Switch to each available unsupported path:

- Non-Qwen backend.
- Custom cloned voice.
- Trained voice.
- Bundled ICL voice.

Expected: the entire emotion/instruct section is hidden and generation omits instruct. Returning to Qwen plus built-in restores the previous text and selection with the editor collapsed.

- [ ] **Step 4: Verify localization behavior**

Select a preset, switch application language, and confirm:

- Labels change language.
- Existing instruct text does not change.
- Clicking another preset inserts that preset’s text in the active UI language.

- [ ] **Step 5: Verify length behavior**

Enter 200 Unicode scalar values and generate.

Expected: generation is allowed.

Enter 201 Unicode scalar values and generate.

Expected: generation is blocked with a localized message; text is not truncated.

- [ ] **Step 6: Run controlled listening comparison**

Use:

- Model: `Qwen3-TTS-12Hz-1.7B-CustomVoice-8bit`
- Speaker: Vivian
- Language: Chinese
- Seed: `4242`
- Identical synthesis text
- Presets: Neutral, Happy, Sad, Angry

Save or overwrite:

```text
/private/tmp/moxin-customvoice-clean-listen/neutral.wav
/private/tmp/moxin-customvoice-clean-listen/happy.wav
/private/tmp/moxin-customvoice-clean-listen/sad.wav
/private/tmp/moxin-customvoice-clean-listen/angry.wav
```

Expected: Angry uses the approved strong instruction and is clearly distinguishable; no abnormal trailing silence, prolonged output, or speaker identity loss.

- [ ] **Step 7: Final verification and dirty-tree audit**

Run:

```bash
git diff --check
cargo check -p moxin-voice-shell
git status --short --branch
```

Expected:

- No whitespace errors.
- Shell compiles.
- All CustomVoice work remains uncommitted and unpushed.
- `.superpowers/` visual-companion artifacts are not staged.

