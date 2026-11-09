//! The language of the interface: English (the texts in the code) or
//! Brazilian Portuguese.
//!
//! The English text is the key: `tr("Open file…")` gives the text in the
//! current language, and `trf!("Could not open {name}.", name = n)` does the
//! same for a text with values. The Portuguese texts are in [`pt`]. A test
//! reads the source files and fails if a text in `tr` or `trf!` has no
//! Portuguese entry, so a new text cannot be forgotten.
//!
//! The language is chosen at start from the system or the browser (any
//! "pt" locale gives Portuguese), and can be changed in the toolbar; the
//! choice is saved.

mod pt;

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;

/// A language of the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Lang {
    #[default]
    En,
    Pt,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Pt];

    /// The name of the language, in that language.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Pt => "Português (Brasil)",
        }
    }

    /// The short code shown on the toolbar button.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "EN",
            Lang::Pt => "PT",
        }
    }

    /// The language for a locale like "pt-BR" or "en_US.UTF-8".
    pub fn from_locale(locale: &str) -> Lang {
        if locale.to_ascii_lowercase().starts_with("pt") {
            Lang::Pt
        } else {
            Lang::En
        }
    }

    /// The language of the system (desktop) or of the browser (web).
    pub fn of_system() -> Lang {
        sys_locale::get_locale().map_or(Lang::En, |l| Lang::from_locale(&l))
    }
}

thread_local! {
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::En) };
}

/// Use `lang` for the texts from now on.
pub fn set(lang: Lang) {
    CURRENT.with(|c| c.set(lang));
}

/// The language in use.
pub fn current() -> Lang {
    CURRENT.with(Cell::get)
}

fn portuguese() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| pt::TEXTS.iter().copied().collect())
}

/// `english` in the current language (`english` itself if there is no
/// translation).
pub fn tr(english: &'static str) -> &'static str {
    match current() {
        Lang::En => english,
        Lang::Pt => portuguese().get(english).copied().unwrap_or(english),
    }
}

/// A text with values: `trf!("{n} words", n = count)`. The template is
/// translated with [`tr`], then each `{name}` is replaced by its value
/// (`Display`). Format the values first if they need a precision.
#[macro_export]
macro_rules! trf {
    ($template:literal $(, $name:ident = $value:expr)* $(,)?) => {{
        #[allow(unused_mut)]
        let mut text = $crate::i18n::tr($template).to_owned();
        $(
            text = text.replace(concat!("{", stringify!($name), "}"), &($value).to_string());
        )*
        text
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales() {
        assert_eq!(Lang::from_locale("pt-BR"), Lang::Pt);
        assert_eq!(Lang::from_locale("pt_PT.UTF-8"), Lang::Pt);
        assert_eq!(Lang::from_locale("en-US"), Lang::En);
        assert_eq!(Lang::from_locale("de"), Lang::En);
    }

    #[test]
    fn translation_and_values() {
        set(Lang::Pt);
        let n = 3;
        assert_eq!(trf!("{n} words", n = n), "3 palavras");
        set(Lang::En);
        assert_eq!(trf!("{n} words", n = n), "3 words");
        assert_eq!(
            tr("no such text, kept as it is"),
            "no such text, kept as it is"
        );
    }

    /// The string literals of `tr("…")` and `trf!("…")` in a source file,
    /// with Rust escapes and line continuations resolved.
    fn keys(src: &str) -> Vec<String> {
        let mut out = Vec::new();
        for start in ["tr(", "trf!("] {
            let mut rest = src;
            while let Some(i) = rest.find(start) {
                // Not part of a longer name (like "attr(").
                let before = rest[..i].chars().next_back();
                rest = &rest[i + start.len()..];
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                // The literal may be on the next line (after cargo fmt).
                let trimmed = rest.trim_start();
                let Some(after_quote) = trimmed.strip_prefix('"') else {
                    continue;
                };
                rest = after_quote;
                let mut value = String::new();
                let mut chars = rest.char_indices().peekable();
                let mut end = rest.len();
                while let Some((k, c)) = chars.next() {
                    match c {
                        '"' => {
                            end = k;
                            break;
                        }
                        '\\' => match chars.next().map(|x| x.1) {
                            Some('\n') => {
                                while chars.peek().is_some_and(|x| x.1.is_whitespace()) {
                                    chars.next();
                                }
                            }
                            Some('n') => value.push('\n'),
                            Some('t') => value.push('\t'),
                            Some('"') => value.push('"'),
                            Some('\\') => value.push('\\'),
                            Some(o) => {
                                value.push('\\');
                                value.push(o);
                            }
                            None => {}
                        },
                        c => value.push(c),
                    }
                }
                rest = &rest[end..];
                out.push(value);
            }
        }
        out
    }

    /// Lists the texts with no translation as JSON (run by hand:
    /// `cargo test dump_missing -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn dump_missing() {
        let _ = std::panic::catch_unwind(every_text_has_a_portuguese_translation);
    }

    #[test]
    fn every_text_has_a_portuguese_translation() {
        let sources = [
            ("app/actions.rs", include_str!("../app/actions.rs")),
            ("app/decoder.rs", include_str!("../app/decoder.rs")),
            ("app/dialogs.rs", include_str!("../app/dialogs.rs")),
            ("app/input.rs", include_str!("../app/input.rs")),
            ("app/menus.rs", include_str!("../app/menus.rs")),
            ("app/mod.rs", include_str!("../app/mod.rs")),
            ("app/plot.rs", include_str!("../app/plot.rs")),
            ("app/readouts.rs", include_str!("../app/readouts.rs")),
            ("measure.rs", include_str!("../measure.rs")),
            ("fft.rs", include_str!("../fft.rs")),
            ("decode/mod.rs", include_str!("../decode/mod.rs")),
            ("decode/uart.rs", include_str!("../decode/uart.rs")),
            ("decode/i2c.rs", include_str!("../decode/i2c.rs")),
            ("decode/spi.rs", include_str!("../decode/spi.rs")),
        ];
        let map = portuguese();
        let mut missing = Vec::new();
        // The examples and their groups (static texts, translated where shown).
        for e in crate::examples::EXAMPLES {
            for k in [e.label, e.about] {
                if !map.contains_key(k) {
                    missing.push(format!("example {}: {k:?}", e.file));
                }
            }
        }
        for g in crate::examples::GROUPS {
            for k in [g.name, g.credit] {
                if !map.contains_key(k) {
                    missing.push(format!("group: {k:?}"));
                }
            }
        }
        for (file, src) in sources {
            // The tests of each file are not translated.
            let src = src.split("#[cfg(test)]").next().unwrap_or(src);
            for k in keys(src) {
                if !map.contains_key(k.as_str()) {
                    missing.push(format!("{file}: {k:?}"));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "no Portuguese text for:\n{}",
            missing.join("\n")
        );
        // And every translation keeps the {values} of its English text.
        for (en, pt) in pt::TEXTS {
            for part in en.split('{').skip(1) {
                let name = part.split('}').next().unwrap_or("");
                assert!(
                    pt.contains(&format!("{{{name}}}")),
                    "{{{name}}} missing in {pt:?}"
                );
            }
        }
    }
}
