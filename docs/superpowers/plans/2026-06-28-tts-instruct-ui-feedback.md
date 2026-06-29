# TTS Instruct UI Feedback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the custom instruction editor visibly editable and keep every emotion option fully visible at narrow TTS panel widths.

**Architecture:** Keep the change inside the existing Makepad live-design tree in `screen.rs`. Reuse the TTS text area's established cursor and selection shaders, and change the emotion control from one fixed horizontal row to a label followed by a wrapping options row.

**Tech Stack:** Rust, Makepad live design, Cargo, macOS visual E2E.

---

### Task 1: Add a visible instruction caret

**Files:**
- Modify: `apps/moxin-voice/src/screen.rs:1968`

- [x] **Step 1: Preserve the reported failing evidence**

Use the supplied screenshot as the failing visual case: the instruction field accepts keyboard input after a click, but no blue caret is visible.

- [x] **Step 2: Reuse the established TTS input drawing contract**

Add these shaders to `instruct_input`, matching the main TTS text input:

```rust
draw_cursor: {
    fn pixel(self) -> vec4 {
        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
        sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 0.5);
        sdf.fill((MOXIN_PRIMARY));
        return sdf.result;
    }
}

draw_selection: {
    fn pixel(self) -> vec4 {
        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
        sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 1.0);
        sdf.fill(vec4(0.39, 0.40, 0.95, 0.2));
        return sdf.result;
    }
}
```

- [x] **Step 3: Compile the application**

Run:

```bash
env CARGO_TARGET_DIR=/Users/alan0x/Documents/projects/moxin-tts/target \
  cargo check -p moxin-voice-shell
```

Expected: `Finished dev profile` with no errors.

### Task 2: Prevent the Custom option from being clipped

**Files:**
- Modify: `apps/moxin-voice/src/screen.rs:1874`

- [x] **Step 1: Preserve the reported failing evidence**

Use the supplied screenshot as the failing visual case: when custom text activates the `Custom` status chip, its right edge extends outside the options container.

- [x] **Step 2: Separate the label from the options**

Change the outer control to a vertical layout:

```rust
emotion_row = <View> {
    width: Fill, height: Fit
    flow: Down
    align: {x: 0.0}
    spacing: 6
```

- [x] **Step 3: Let the option chips wrap within available width**

Change the option container to:

```rust
emotion_options = <View> {
    width: Fill, height: Fit
    flow: Right { wrap: true }
    align: {y: 0.5}
    spacing: 4
```

- [x] **Step 4: Compile and run focused tests**

Run:

```bash
env CARGO_TARGET_DIR=/Users/alan0x/Documents/projects/moxin-tts/target \
  cargo test -p moxin-voice tts_emotion
```

Expected: all 10 emotion tests pass.

Run:

```bash
git diff --check
```

Expected: no output.

### Task 3: Verify the two visual regressions

**Files:**
- Test: `apps/moxin-voice/src/screen.rs`

- [x] **Step 1: Rebuild and launch the shared application binary**

Run:

```bash
env CARGO_TARGET_DIR=/Users/alan0x/Documents/projects/moxin-tts/target \
  cargo build -p moxin-voice-shell
```

Expected: `Finished dev profile`.

- [ ] **Step 2: Verify the input focus state**

Open Text to Speech, expand the instruction editor, click inside the input, and type text. Expected: a blue caret is visible at the insertion point and the entered text remains editable.

- [ ] **Step 3: Verify the Custom state layout**

Enter custom text at the same window width as the reported screenshot. Expected: `Custom` is fully visible; when horizontal space is insufficient, chips continue on a new line without overlapping or clipping.

- [x] **Step 4: Leave the fix uncommitted for user E2E**

Do not create another commit until the user confirms the corrected UI behavior.
