# CustomVoice Instruct Editor Design

Date: 2026-06-24

## Goal

Expose Qwen3-TTS CustomVoice `instruct` to users without replacing the fast preset workflow.

The interface provides:

- Built-in emotion presets.
- An expandable editor for the final instruction.
- A Custom state whenever the user changes preset text.
- Full visibility only when the selected backend and voice support CustomVoice instruct.

The final editable instruction is the single source of truth sent to the model.

## Scope

Included:

- TTS emotion preset controls.
- Expandable instruct editor.
- Preset, Custom, and Neutral state transitions.
- UI payload and Dora propagation.
- History persistence and restoration.
- Validation and regression tests.
- Updated Angry preset based on the approved listening test.

Excluded:

- VoiceDesign integration.
- Instruct support for cloned, trained, or bundled ICL voices.
- Automatic speed, pitch, or volume changes based on emotion.
- User-created reusable preset libraries.
- Synchronizing instruct defaults through global application preferences.

## Chosen UX

The selected layout is the compact, expandable editor.

The normal state shows:

- Emotion preset controls.
- A one-line summary of the active instruct when it is non-empty.
- An Edit instruction command.

The multi-line editor is only shown after the user opens it. Collapsing the editor does not modify the text.

The Edit instruction command remains available in Neutral so a user can create a
Custom instruction without selecting a preset first.

### Presets

The first version contains:

- Neutral
- Happy
- Angry
- Sad
- Gentle
- Excited
- Custom, shown as the selected state after manual editing

Custom is a status, not a clickable preset. It appears selected only while the
current text is manually authored or no longer exactly represents the selected
preset.

Clicking a preset immediately replaces the current instruct text with that preset's localized value.

Clicking Neutral clears the instruct.

Clicking a preset while Custom is selected discards the Custom text and restores the selected preset text.

### Manual Editing

When the user edits text that came from a preset:

- The selected state immediately changes to Custom.
- The edited text becomes the final value used for synthesis.
- The original preset remains unchanged.

When the editor becomes exactly empty:

- The selected state changes to Neutral.
- No effective instruct is sent.

Whitespace-only text remains Custom and is sent unchanged. The UI and backend must not silently trim or rewrite user input.

## Availability

The entire emotion and instruct section is visible only when both conditions are true:

1. The active inference backend is Qwen3-TTS MLX.
2. The selected voice is a built-in Qwen CustomVoice speaker.

The section is hidden for:

- PrimeSpeech and other inference backends.
- Custom cloned voices.
- Trained voices.
- Bundled ICL voices.

Hiding the section does not make instruct available to unsupported paths. A generation request from an unsupported path must omit instruct.

The current text and selection are preserved in memory while the section is
hidden, so temporarily switching voices does not destroy user input. The editor
collapses when hidden. If the user returns to a supported backend and built-in
voice, the preserved text and selection reappear; until then, no instruct is
included in generation requests.

## State Model

The UI keeps three independent pieces of state:

```rust
enum TtsInstructSelection {
    Neutral,
    Preset(&'static str),
    Custom,
}

struct TtsInstructState {
    selection: TtsInstructSelection,
    text: String,
    editor_expanded: bool,
}
```

`text` is the only synthesis value. `selection` controls presentation and history metadata. `editor_expanded` controls layout only.

The implementation may use equivalent existing widget state rather than introducing these exact Rust types, but the behavior must remain the same.

## Preset Data

Each preset contains:

- Stable ID.
- Chinese label.
- English label.
- Chinese instruct.
- English instruct.

Neutral has no instruct.

The approved Angry defaults are:

```text
Chinese:
用极其愤怒、严厉斥责、情绪爆发的语气说，语调强烈，咬字有力

English:
Use an extremely angry, stern, emotionally explosive tone, with forceful intonation and articulation.
```

The other presets retain their current mappings:

```text
Happy:
  Chinese: 用开心、轻快的语气说
  English: Say it in a happy and lively tone
Sad:
  Chinese: 用难过、低落的语气说
  English: Say it in a sad and subdued tone
Gentle:
  Chinese: 用温柔、平静的语气说
  English: Say it in a gentle and calm tone
Excited:
  Chinese: 用兴奋、充满活力的语气说
  English: Say it in an excited and energetic tone
```

## Interaction Rules

### Initialization

- Start in Neutral.
- Instruct text is empty.
- The editor is collapsed.

### Preset Click

1. Replace instruct text with the preset's localized text.
2. Select that preset.
3. Keep the editor's current expanded or collapsed state.
4. Refresh the visible summary.

Changing the application language updates labels but does not silently rewrite
the current instruct text. Clicking a preset after the language change inserts
that preset's instruct in the newly active UI language.

### Edit

1. Update instruct text on each edit event.
2. If the text is exactly empty, select Neutral.
3. Otherwise select Custom.
4. Do not mutate preset definitions.

### Collapse

- Hide the multi-line editor.
- Preserve text and selection.
- Show a single-line truncated summary when instruct is non-empty.

### Generate

- Read the current instruct text directly.
- Send `None` only when the text is exactly empty.
- Do not regenerate instruct from the selected preset during submission.

## Input Constraints

- Maximum length: 200 Unicode scalar values.
- Multi-line input is allowed.
- The editor should show the remaining character count only when near the limit or over the limit.
- Generation is blocked when the text exceeds the limit.
- The existing application toast/error pattern is used for the validation message.
- No content filtering or automatic prompt rewriting is performed locally.

## Data Flow

```text
Preset click or manual edit
    -> UI instruct state
    -> generation snapshot
    -> JSON payload.instruct
    -> Dora TtsParams.instruct
    -> SynthesizeOptions.instruct
    -> separate ChatML user instruct tokens
    -> official CustomVoice non-streaming prefill
```

The payload remains backward-compatible:

```json
{
  "prompt": "VOICE:vivian|text",
  "speed": 1.0,
  "pitch": 0.0,
  "volume": 100.0,
  "emotion": "custom",
  "instruct": "user-edited instruction"
}
```

`emotion` is UI and history metadata. `instruct` is the actual model input.

## History

Each history entry stores:

- `emotion_id`
- `emotion_label`
- The final `instruct` text

Old history entries without these optional fields remain valid.

When restoring a history entry:

1. Restore speed, pitch, volume, and final instruct text.
2. If instruct is exactly empty or absent, restore Neutral.
3. If instruct exactly matches any current localized text variant of a preset,
   restore that preset regardless of the active UI language.
4. Otherwise restore Custom.
5. Keep the editor collapsed initially.

History playback uses the saved audio and does not regenerate automatically. Regeneration uses the restored final instruct text.

## Error Handling

- Unsupported backend or voice: hide the feature and omit instruct.
- Over-length instruct: block generation and show a localized validation message.
- Backend synthesis error: use the existing generation failure path.
- Do not silently retry with Neutral.
- Do not silently route CustomVoice instruct to VoiceDesign.

## Testing

### Preset Unit Tests

- Neutral maps to no instruct.
- Happy and Sad map to their expected localized instruct.
- Angry maps to the approved strong instruct.
- Unknown preset IDs fall back to Neutral.

### State Transition Tests

- Clicking a preset replaces previous Custom text.
- Editing preset text switches selection to Custom.
- Clearing text switches selection to Neutral.
- Collapsing the editor preserves text.
- Whitespace-only input remains Custom.
- Changing the application language does not rewrite the current instruct.
- Clicking a preset inserts the preset text for the active UI language.
- Custom is displayed as a state and cannot be clicked as a preset action.

### Availability Tests

- Qwen backend plus built-in voice shows the feature.
- Non-Qwen backend hides the feature.
- Cloned, trained, and bundled ICL voices hide the feature.
- Unsupported generation paths omit instruct.
- Hidden state preserves text and selection but collapses the editor.
- Returning to a supported path restores the preserved state.

### Payload Tests

- Payload uses the final editable text.
- Empty text produces no effective instruct.
- Manual edits are not replaced by preset text during submission.
- Values up to 200 characters are accepted.
- Values over 200 characters block generation.

### History Tests

- Legacy history without emotion fields deserializes.
- Any localized variant of a preset instruct restores the matching preset.
- Edited instruct restores Custom.
- Empty instruct restores Neutral.

### Backend Regression Tests

- Dora parses and preserves non-empty instruct.
- CustomVoice instruct remains on the CustomVoice route.
- VoiceDesign routing is selected only by a VoiceDesign model.
- ChatML added special tokens retain official IDs.
- Official assistant `3:-5` target slicing remains covered.
- Main talker and sub-talker sampling configuration remains covered.

### Listening Test

Use the same:

- Model
- Built-in speaker
- Language
- Text
- Random seed

Compare:

- Neutral
- Happy
- Sad
- Angry

The accepted Vivian Chinese baseline uses seed `4242`. The Angry preset must remain clearly distinguishable without abnormal long output, trailing silence, or speaker identity loss.

## Acceptance Criteria

- Users can synthesize with presets without opening the editor.
- Users can inspect and edit the exact instruct sent to the model.
- Editing switches the selected state to Custom.
- Unsupported paths do not display or send instruct.
- History restores the final instruct deterministically.
- Neutral, Happy, Sad, and the strong Angry preset remain audibly distinguishable in controlled listening tests.
- No VoiceDesign code or model dependency is required for this feature.
