pub const NEUTRAL_EMOTION_ID: &str = "neutral";
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

impl TtsInstructState {
    pub fn from_preset(id: &str, english: bool) -> Self {
        let mut state = Self::default();
        state.select_preset(id, english);
        state
    }

    pub fn from_history(instruct: Option<&str>) -> Self {
        let Some(instruct) = instruct else {
            return Self::default();
        };
        if instruct.is_empty() {
            return Self::default();
        }

        if let Some(preset) = EMOTION_PRESETS.iter().find(|preset| {
            preset.instruct_zh == Some(instruct) || preset.instruct_en == Some(instruct)
        }) {
            return Self {
                selection: TtsInstructSelection::Preset(preset.id),
                text: instruct.to_string(),
                editor_expanded: false,
            };
        }

        Self {
            selection: TtsInstructSelection::Custom,
            text: instruct.to_string(),
            editor_expanded: false,
        }
    }

    pub fn select_preset(&mut self, id: &str, english: bool) {
        let preset = emotion_preset_or_neutral(id);
        self.text = preset.instruct(english).unwrap_or_default().to_string();
        self.selection = if preset.id == NEUTRAL_EMOTION_ID {
            TtsInstructSelection::Neutral
        } else {
            TtsInstructSelection::Preset(preset.id)
        };
    }

    pub fn edit(&mut self, text: String) {
        self.selection = if text.is_empty() {
            TtsInstructSelection::Neutral
        } else {
            TtsInstructSelection::Custom
        };
        self.text = text;
    }

    pub fn collapse(&mut self) {
        self.editor_expanded = false;
    }

    pub fn toggle_editor(&mut self) {
        self.editor_expanded = !self.editor_expanded;
    }

    pub fn selection(&self) -> TtsInstructSelection {
        self.selection
    }

    pub fn selection_id(&self) -> &'static str {
        match self.selection {
            TtsInstructSelection::Neutral => NEUTRAL_EMOTION_ID,
            TtsInstructSelection::Preset(id) => id,
            TtsInstructSelection::Custom => CUSTOM_EMOTION_ID,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn editor_expanded(&self) -> bool {
        self.editor_expanded
    }

    pub fn effective_instruct(&self) -> Option<&str> {
        (!self.text.is_empty()).then_some(self.text.as_str())
    }

    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    pub fn is_over_limit(&self) -> bool {
        self.char_count() > TTS_INSTRUCT_MAX_CHARS
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TtsEmotionPreset {
    pub id: &'static str,
    label_zh: &'static str,
    label_en: &'static str,
    instruct_zh: Option<&'static str>,
    instruct_en: Option<&'static str>,
}

impl TtsEmotionPreset {
    pub fn label(&self, english: bool) -> &'static str {
        if english {
            self.label_en
        } else {
            self.label_zh
        }
    }

    pub fn instruct(&self, english: bool) -> Option<&'static str> {
        if english {
            self.instruct_en.or(self.instruct_zh)
        } else {
            self.instruct_zh
        }
    }
}

pub const EMOTION_PRESETS: &[TtsEmotionPreset] = &[
    TtsEmotionPreset {
        id: NEUTRAL_EMOTION_ID,
        label_zh: "自然",
        label_en: "Neutral",
        instruct_zh: None,
        instruct_en: None,
    },
    TtsEmotionPreset {
        id: "happy",
        label_zh: "开心",
        label_en: "Happy",
        instruct_zh: Some("用开心、轻快的语气说"),
        instruct_en: Some("Say it in a happy and lively tone"),
    },
    TtsEmotionPreset {
        id: "angry",
        label_zh: "愤怒",
        label_en: "Angry",
        instruct_zh: Some("用极其愤怒、严厉斥责、情绪爆发的语气说，语调强烈，咬字有力"),
        instruct_en: Some(
            "Use an extremely angry, stern, emotionally explosive tone, with forceful intonation and articulation.",
        ),
    },
    TtsEmotionPreset {
        id: "sad",
        label_zh: "难过",
        label_en: "Sad",
        instruct_zh: Some("用难过、低落的语气说"),
        instruct_en: Some("Say it in a sad and subdued tone"),
    },
    TtsEmotionPreset {
        id: "gentle",
        label_zh: "温柔",
        label_en: "Gentle",
        instruct_zh: Some("用温柔、平静的语气说"),
        instruct_en: Some("Say it in a gentle and calm tone"),
    },
    TtsEmotionPreset {
        id: "excited",
        label_zh: "兴奋",
        label_en: "Excited",
        instruct_zh: Some("用兴奋、充满活力的语气说"),
        instruct_en: Some("Say it in an excited and energetic tone"),
    },
];

pub fn emotion_preset(id: &str) -> Option<&'static TtsEmotionPreset> {
    EMOTION_PRESETS.iter().find(|preset| preset.id == id)
}

pub fn emotion_preset_or_neutral(id: &str) -> &'static TtsEmotionPreset {
    emotion_preset(id).unwrap_or(&EMOTION_PRESETS[0])
}

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

pub fn selection_for_request(
    state: &TtsInstructState,
    backend: &str,
    is_builtin_voice: bool,
) -> TtsInstructSelection {
    if supports_custom_voice_instruct(backend, is_builtin_voice) {
        state.selection()
    } else {
        TtsInstructSelection::Neutral
    }
}

pub fn instruct_is_over_limit_for_request(
    state: &TtsInstructState,
    backend: &str,
    is_builtin_voice: bool,
) -> bool {
    supports_custom_voice_instruct(backend, is_builtin_voice) && state.is_over_limit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_emotion_has_no_native_instruct() {
        let preset = emotion_preset("neutral").unwrap();

        assert_eq!(preset.instruct(false), None);
        assert_eq!(preset.instruct(true), None);
    }

    #[test]
    fn happy_emotion_maps_to_native_instruct() {
        let preset = emotion_preset("happy").unwrap();

        assert_eq!(preset.label(false), "开心");
        assert_eq!(preset.label(true), "Happy");
        assert_eq!(preset.instruct(false), Some("用开心、轻快的语气说"));
    }

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
        assert_eq!(
            state.selection(),
            TtsInstructSelection::Preset("happy")
        );
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

    #[test]
    fn unsupported_request_omits_preserved_instruct() {
        let state = TtsInstructState::from_preset("happy", false);

        assert_eq!(
            instruct_for_request(&state, "qwen3_tts_mlx", true),
            Some("用开心、轻快的语气说")
        );
        assert_eq!(instruct_for_request(&state, "primespeech", true), None);
        assert_eq!(
            instruct_for_request(&state, "qwen3_tts_mlx", false),
            None
        );
    }

    #[test]
    fn unsupported_request_ignores_hidden_validation_and_uses_neutral_metadata() {
        let mut state = TtsInstructState::default();
        state.edit("情".repeat(TTS_INSTRUCT_MAX_CHARS + 1));

        assert!(!instruct_is_over_limit_for_request(
            &state,
            "qwen3_tts_mlx",
            false
        ));
        assert_eq!(
            selection_for_request(&state, "qwen3_tts_mlx", false),
            TtsInstructSelection::Neutral
        );
        assert!(instruct_is_over_limit_for_request(
            &state,
            "qwen3_tts_mlx",
            true
        ));
        assert_eq!(
            selection_for_request(&state, "qwen3_tts_mlx", true),
            TtsInstructSelection::Custom
        );
    }

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
            TtsInstructState::from_history(Some("Very restrained, but clearly hopeful."))
                .selection(),
            TtsInstructSelection::Custom
        );
    }
}
