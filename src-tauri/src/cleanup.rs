//! Deterministic transcript cleanup — the always-on layer of the SPEC §4 pipeline.
//!
//! `clean()` is a pure function: (transcript, options) -> cleaned transcript.
//! It strips filler words, bracketed non-speech artifacts, optionally collapses
//! immediately-repeated words, and normalizes whitespace/punctuation damage
//! caused by the removals.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Default filler-word list (SPEC §4.1). User-editable via settings.
pub const DEFAULT_FILLERS: &[&str] = &[
    "uh", "um", "uhm", "umm", "uhh", "hmm", "hm", "mm", "mmm", "mhm", "er", "erm", "ah", "eh",
];

/// Personal-dictionary caps (SPEC7 FR-D1): entries past these are ignored
/// with a warning, never a crash.
pub const MAX_DICTIONARY_ENTRIES: usize = 200;
pub const MAX_DICTIONARY_FIELD_CHARS: usize = 100;

/// A personal-dictionary replacement (SPEC7 FR-D): `from` is a literal
/// whole-word phrase (multi-word allowed, never regex), `to` is inserted
/// verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub from: String,
    pub to: String,
}

/// Non-speech artifact content emitted by STT models inside [] or ().
static ARTIFACT_CONTENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)^\s*(music|laugh(s|ter|ing)?|cough(s|ing)?|applause|noise|silence|blank\s*audio|inaudible|unintelligible|sigh(s|ing)?|breath(s|ing)?|clears?\s+throat|clicks?|beep(s|ing)?|static|typing|pause|crosstalk|speaking\s+in\s+foreign\s+language|foreign\s+language|\s*)\s*$",
    )
    .unwrap()
});

static BRACKET_SPAN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[([^\[\]]*)\]|\(([^()]*)\)").unwrap());

#[derive(Debug, Clone)]
pub struct CleanOptions {
    /// Lowercase filler words to strip (whole-word, case-insensitive match).
    pub fillers: Vec<String>,
    /// Collapse immediately-repeated words ("the the" -> "the").
    pub collapse_repeats: bool,
    /// Personal-dictionary replacements, applied last in list order (SPEC7 FR-D2).
    pub dictionary: Vec<DictionaryEntry>,
}

impl Default for CleanOptions {
    fn default() -> Self {
        Self {
            fillers: DEFAULT_FILLERS.iter().map(|s| s.to_string()).collect(),
            collapse_repeats: true,
            dictionary: Vec::new(),
        }
    }
}

/// A token split into leading punctuation, core word, and trailing punctuation.
struct Token {
    lead: String,
    core: String,
    trail: String,
}

impl Token {
    fn parse(raw: &str) -> Self {
        let start = raw
            .char_indices()
            .find(|(_, c)| c.is_alphanumeric())
            .map(|(i, _)| i);
        let end = raw
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_alphanumeric())
            .map(|(i, c)| i + c.len_utf8());
        match (start, end) {
            (Some(s), Some(e)) => Self {
                lead: raw[..s].to_string(),
                core: raw[s..e].to_string(),
                trail: raw[e..].to_string(),
            },
            _ => Self {
                lead: raw.to_string(),
                core: String::new(),
                trail: String::new(),
            },
        }
    }

    fn render(&self) -> String {
        format!("{}{}{}", self.lead, self.core, self.trail)
    }

    fn ends_sentence(&self) -> bool {
        self.trail.contains(['.', '!', '?'])
    }
}

fn capitalize_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Strip bracketed non-speech artifacts like `[music]`, `(coughs)`, `[BLANK_AUDIO]`.
fn strip_artifacts(text: &str) -> String {
    BRACKET_SPAN
        .replace_all(text, |caps: &regex::Captures| {
            let content = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str())
                .unwrap_or("");
            // Normalize underscores so BLANK_AUDIO-style tags match word patterns.
            let normalized = content.replace('_', " ");
            if ARTIFACT_CONTENT.is_match(&normalized) {
                String::new()
            } else {
                caps.get(0).unwrap().as_str().to_string()
            }
        })
        .into_owned()
}

/// Apply the personal dictionary (SPEC7 FR-D2): each entry's `from` is a
/// literal phrase matched case-insensitively on whole-word boundaries;
/// `to` is inserted verbatim (no capture-group expansion). Entries apply in
/// list order, each over the whole text. An empty dictionary is a no-op.
fn apply_dictionary(text: &str, entries: &[DictionaryEntry]) -> String {
    let mut out = text.to_string();
    for (index, entry) in entries.iter().enumerate() {
        if index >= MAX_DICTIONARY_ENTRIES {
            log::warn!(
                "personal dictionary capped at {MAX_DICTIONARY_ENTRIES} entries; the rest are ignored"
            );
            break;
        }
        let from = entry.from.trim();
        if from.is_empty() {
            continue;
        }
        if from.chars().count() > MAX_DICTIONARY_FIELD_CHARS
            || entry.to.chars().count() > MAX_DICTIONARY_FIELD_CHARS
        {
            log::warn!("personal dictionary entry '{from}' exceeds the length cap; ignored");
            continue;
        }
        // The phrase is escaped, so user input is never regex syntax. Word
        // boundaries only bind next to word characters — a phrase edged in
        // punctuation still matches literally.
        let mut pattern = String::from("(?i)");
        if from.chars().next().is_some_and(|c| c.is_alphanumeric()) {
            pattern.push_str(r"\b");
        }
        pattern.push_str(&regex::escape(from));
        if from.chars().last().is_some_and(|c| c.is_alphanumeric()) {
            pattern.push_str(r"\b");
        }
        match Regex::new(&pattern) {
            Ok(re) => out = re.replace_all(&out, regex::NoExpand(&entry.to)).into_owned(),
            Err(e) => log::warn!("personal dictionary entry '{from}' unusable: {e}"),
        }
    }
    out
}

pub fn clean(text: &str, opts: &CleanOptions) -> String {
    let text = strip_artifacts(text);
    let fillers: Vec<String> = opts.fillers.iter().map(|f| f.to_lowercase()).collect();

    let mut out: Vec<Token> = Vec::new();
    // True when the next kept word begins a sentence (start of text or after .!?).
    let mut at_sentence_start = true;
    // Casing is preserved EXCEPT when a removed filler opened a sentence — then
    // the next kept word is capitalized so the sentence still starts properly.
    let mut capitalize_next = false;
    // Set when a removed filler carried terminal punctuation that must survive.
    let mut pending_terminal: Option<char> = None;

    for raw in text.split_whitespace() {
        let mut tok = Token::parse(raw);
        if tok.core.is_empty() {
            // Pure punctuation fragment (e.g. stranded "...") — keep it attached
            // to the previous token if any, else drop.
            if let Some(prev) = out.last_mut() {
                prev.trail.push_str(&tok.lead);
            }
            continue;
        }

        let core_lower = tok.core.to_lowercase();

        if fillers.iter().any(|f| *f == core_lower) {
            // Removed filler: keep sentence state, propagate terminal punctuation.
            if at_sentence_start {
                capitalize_next = true;
            }
            if let Some(c) = tok.trail.chars().find(|c| ['.', '!', '?'].contains(c)) {
                if let Some(prev) = out.last_mut() {
                    if !prev.ends_sentence() {
                        // Replace a trailing comma with the terminal punct if present.
                        if prev.trail.starts_with(',') {
                            prev.trail = prev.trail.replacen(',', &c.to_string(), 1);
                        } else {
                            prev.trail.push(c);
                        }
                    }
                    at_sentence_start = true;
                } else {
                    pending_terminal = None; // terminal punct with nothing before it: drop
                }
            }
            continue;
        }

        if let Some(c) = pending_terminal.take() {
            tok.lead = format!("{}{}", c, tok.lead);
        }

        // Repeated-word collapse: previous kept token has the same core
        // (case-insensitive on this, the second, occurrence) and no trailing
        // punctuation separating them.
        if opts.collapse_repeats {
            if let Some(prev) = out.last_mut() {
                if !prev.core.is_empty()
                    && prev.trail.is_empty()
                    && tok.lead.is_empty()
                    && prev.core.to_lowercase() == core_lower
                {
                    // Keep the first occurrence; adopt the second's trailing punct.
                    prev.trail = tok.trail.clone();
                    at_sentence_start = prev.ends_sentence();
                    continue;
                }
            }
        }

        if capitalize_next {
            tok.core = capitalize_first(&tok.core);
            capitalize_next = false;
        }
        at_sentence_start = tok.ends_sentence();
        out.push(tok);
    }

    let joined = out
        .iter()
        .map(Token::render)
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    // Dictionary runs last (SPEC7 FR-D2) so replacements see the final text —
    // and enhancement, which runs after clean(), sees the corrected names.
    if opts.dictionary.is_empty() {
        joined
    } else {
        apply_dictionary(&joined, &opts.dictionary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(text: &str) -> String {
        clean(text, &CleanOptions::default())
    }

    // R1: filler removal — default list, word boundaries, capitalization repair,
    // filler+comma repair.
    #[test]
    fn r1_strips_default_fillers_whole_word() {
        assert_eq!(c("I was um thinking"), "I was thinking");
        assert_eq!(c("uh let's go"), "Let's go");
        // Mid-sentence filler removal never touches casing.
        assert_eq!(c("so hmm what now"), "so what now");
    }

    #[test]
    fn r1_word_boundaries_protect_real_words() {
        assert_eq!(c("my umbrella is here"), "my umbrella is here");
        assert_eq!(c("summer is warm"), "summer is warm");
        assert_eq!(c("the era of ahoy"), "the era of ahoy");
    }

    #[test]
    fn r1_leading_filler_capitalization_repair() {
        assert_eq!(c("um, hello there"), "Hello there");
        assert_eq!(c("Great. um, next point"), "Great. Next point");
    }

    #[test]
    fn r1_filler_comma_repair() {
        assert_eq!(c("I was, um, thinking"), "I was, thinking");
        assert_eq!(c("yes, uh, I agree"), "yes, I agree");
        // Filler carrying the sentence's terminal punctuation.
        assert_eq!(c("stop it, um."), "stop it.");
    }

    #[test]
    fn r1_custom_filler_list() {
        let opts = CleanOptions {
            fillers: vec!["like".into()],
            ..Default::default()
        };
        assert_eq!(clean("it was like huge", &opts), "it was huge");
        // Default fillers no longer stripped with a custom list.
        assert_eq!(clean("um hello", &opts), "um hello");
    }

    // R2: bracketed artifact stripping.
    #[test]
    fn r2_strips_known_artifacts() {
        // Artifacts are stripped in a pre-pass and never affect casing.
        assert_eq!(c("hello [music] world"), "hello world");
        assert_eq!(c("(coughs) as I said"), "as I said");
        assert_eq!(c("[BLANK_AUDIO]"), "");
        assert_eq!(c("done [applause] now (laughter) ok"), "done now ok");
    }

    #[test]
    fn r2_keeps_real_parenthetical_content() {
        assert_eq!(
            c("the result (see appendix) is fine"),
            "the result (see appendix) is fine"
        );
        assert_eq!(c("array[index] notation"), "array[index] notation");
    }

    // R3: repeated-word collapse.
    #[test]
    fn r3_collapses_immediate_repeats() {
        assert_eq!(c("the the store"), "the store");
        assert_eq!(c("The the store"), "The store");
        assert_eq!(c("go go go"), "go");
    }

    #[test]
    fn r3_preserved_when_setting_off() {
        let opts = CleanOptions {
            collapse_repeats: false,
            ..Default::default()
        };
        assert_eq!(clean("he had had enough", &opts), "he had had enough");
    }

    #[test]
    fn r3_no_collapse_across_sentence_punctuation() {
        assert_eq!(c("I know. Know what?"), "I know. Know what?");
    }

    // R4: edge cases.
    #[test]
    fn r4_empty_and_all_filler() {
        assert_eq!(c(""), "");
        assert_eq!(c("   "), "");
        assert_eq!(c("um uh umm hmm"), "");
        assert_eq!(c("um, uh."), "");
    }

    #[test]
    fn r4_whitespace_collapse() {
        assert_eq!(c("hello    world\n\nagain"), "hello world again");
        assert_eq!(c("  padded  "), "padded");
    }

    #[test]
    fn r4_punctuation_stranding_repaired() {
        // Stranded ellipsis fragment after a removed filler attaches to prior word.
        assert_eq!(c("well um ... fine"), "well... fine");
        // Artifact removal never leaves doubled spaces.
        assert_eq!(c("a [music]  b"), "a b");
    }

    #[test]
    fn r4_unicode_safe() {
        assert_eq!(c("café um münchen"), "café münchen");
        assert_eq!(c("um über alles"), "Über alles");
    }

    // R12: personal dictionary (SPEC7 FR-D).
    fn dict(entries: &[(&str, &str)]) -> CleanOptions {
        CleanOptions {
            dictionary: entries
                .iter()
                .map(|(from, to)| DictionaryEntry {
                    from: (*from).into(),
                    to: (*to).into(),
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn r12_case_insensitive_whole_word_replacement() {
        let opts = dict(&[("acme corp", "AcmeCorp")]);
        assert_eq!(clean("I emailed acme corp today", &opts), "I emailed AcmeCorp today");
        assert_eq!(clean("ACME CORP rocks", &opts), "AcmeCorp rocks");
    }

    #[test]
    fn r12_multi_word_phrase() {
        let opts = dict(&[("jorge pereira", "Jorge Pereira")]);
        assert_eq!(
            clean("send it to jorge pereira please", &opts),
            "send it to Jorge Pereira please"
        );
    }

    #[test]
    fn r12_no_partial_word_match() {
        let opts = dict(&[("cat", "feline")]);
        assert_eq!(clean("the catalog lists a cat", &opts), "the catalog lists a feline");
        assert_eq!(clean("concatenate the strings", &opts), "concatenate the strings");
    }

    #[test]
    fn r12_from_is_literal_not_regex() {
        // '.' and '(' in `from` must match literally, never as regex syntax.
        let opts = dict(&[("node.js", "Node.js")]);
        assert_eq!(clean("we use node.js here", &opts), "we use Node.js here");
        assert_eq!(clean("a nodeXjs impostor", &opts), "a nodeXjs impostor");
        let opts = dict(&[("c(x)", "c of x")]);
        assert_eq!(clean("compute c(x) now", &opts), "compute c of x now");
    }

    #[test]
    fn r12_replacement_is_literal_no_expansion() {
        // '$' in `to` is inserted verbatim, never expanded as a capture group.
        let opts = dict(&[("ten dollars", "$10")]);
        assert_eq!(clean("that costs ten dollars now", &opts), "that costs $10 now");
    }

    #[test]
    fn r12_entries_apply_in_list_order() {
        let opts = dict(&[("alpha", "beta"), ("beta", "gamma")]);
        // First entry rewrites, then the second sees its output.
        assert_eq!(clean("alpha", &opts), "gamma");
    }

    #[test]
    fn r12_blank_from_entries_are_ignored() {
        let opts = dict(&[("", "nope"), ("   ", "nope"), ("ok", "fine")]);
        assert_eq!(clean("all ok here", &opts), "all fine here");
    }

    #[test]
    fn r12_caps_enforced() {
        // Entry #201 is ignored (SPEC7 FR-D1).
        let mut entries: Vec<(String, String)> = (0..MAX_DICTIONARY_ENTRIES)
            .map(|i| (format!("inert{i}"), format!("unused{i}")))
            .collect();
        entries.push(("target".into(), "replaced".into()));
        let opts = CleanOptions {
            dictionary: entries
                .into_iter()
                .map(|(from, to)| DictionaryEntry { from, to })
                .collect(),
            ..Default::default()
        };
        assert_eq!(clean("the target stands", &opts), "the target stands");

        // Over-long `from`/`to` fields are ignored, not truncated.
        let long = "x".repeat(MAX_DICTIONARY_FIELD_CHARS + 1);
        let opts = dict(&[(long.as_str(), "short")]);
        assert_eq!(clean(&format!("keep {long} intact"), &opts), format!("keep {long} intact"));
    }

    #[test]
    fn r12_empty_dictionary_is_identity() {
        // Byte-identical to the non-dictionary pipeline output.
        let text = "so um the the plan (coughs) is fine";
        assert_eq!(clean(text, &dict(&[])), c(text));
    }

    #[test]
    fn r12_runs_after_fillers_and_repeats() {
        // The phrase only assembles once fillers are stripped from its middle.
        let opts = dict(&[("jorge pereira", "Jorge Pereira")]);
        assert_eq!(
            clean("email jorge um pereira now", &opts),
            "email Jorge Pereira now"
        );
        // Consequence of the SPEC7 FR-D2 ordering: repeat collapsing runs
        // FIRST, so a phrase made of an immediately repeated word ("yat yat")
        // has already collapsed to one word by dictionary time and cannot
        // match while collapse_repeats is on.
        let opts = CleanOptions {
            dictionary: vec![DictionaryEntry {
                from: "yat yat".into(),
                to: "Yat Yat".into(),
            }],
            ..Default::default()
        };
        assert_eq!(clean("open yat yat now", &opts), "open yat now");
        let opts = CleanOptions {
            collapse_repeats: false,
            ..opts
        };
        assert_eq!(clean("open yat yat now", &opts), "open Yat Yat now");
    }
}
