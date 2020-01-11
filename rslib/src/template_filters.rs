// Copyright: Ankitects Pty Ltd and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

use crate::text::strip_html;
use blake3::Hasher;
use lazy_static::lazy_static;
use regex::{Captures, Regex};
use std::borrow::Cow;

// Filtering
//----------------------------------------

/// Applies built in filters, returning the resulting text and remaining filters.
///
/// The first non-standard filter that is encountered will terminate processing,
/// so non-standard filters must come at the end.
pub(crate) fn apply_filters<'a>(
    text: &'a str,
    filters: &[&str],
    field_name: &str,
) -> (Cow<'a, str>, Vec<String>) {
    let mut text: Cow<str> = text.into();

    // type:cloze is handled specially
    let filters = if filters == ["type", "cloze"] {
        &["type-cloze"]
    } else {
        filters
    };

    for (idx, &filter_name) in filters.iter().enumerate() {
        match apply_filter(filter_name, text.as_ref(), field_name) {
            (true, None) => {
                // filter did not change text
            }
            (true, Some(output)) => {
                // text updated
                text = output.into();
            }
            (false, _) => {
                // unrecognized filter, return current text and remaining filters
                return (
                    text,
                    filters.iter().skip(idx).map(ToString::to_string).collect(),
                );
            }
        }
    }

    // all filters processed
    (text, vec![])
}

/// Apply one filter.
///
/// Returns true if filter was valid.
/// Returns string if input text changed.
fn apply_filter<'a>(filter_name: &str, text: &'a str, field_name: &str) -> (bool, Option<String>) {
    let output_text = match filter_name {
        "text" => strip_html(text),
        "furigana" => furigana_filter(text),
        "kanji" => kanji_filter(text),
        "kana" => kana_filter(text),
        other => {
            let split: Vec<_> = other.splitn(2, '-').collect();
            let base = split[0];
            let filter_args = *split.get(1).unwrap_or(&"");
            match base {
                "type" => type_filter(text, filter_args, field_name),
                "hint" => hint_filter(text, field_name),
                //"cq" => cloze_filter(text, filter_args, true),
                //"ca" => cloze_filter(text, filter_args, false),
                _ => return (false, None),
            }
        }
    };

    (
        true,
        match output_text {
            Cow::Owned(o) => Some(o),
            _ => None,
        },
    )
}

// Cloze filter
//----------------------------------------

lazy_static! {
    static ref CLOZE: Regex = Regex::new(
        r#"(?xsi)
                \{\{
                (c)(\d+)::  # 1 = c or C, 2 = cloze number
                (.*?) # 3 = clozed text
                (?:
                  ::(.*?) # 4 = optional hint
                )?
                \}\}
                "#
    )
    .unwrap();
}

mod cloze_caps {
    // the lower or uppercase C in the cloze deletion
    pub static C_CHAR: usize = 1;
    // cloze ordinal
    pub static ORD: usize = 2;
    // the occluded text
    pub static TEXT: usize = 3;
    // optional hint
    pub static HINT: usize = 4;
}

fn reveal_cloze_text(text: &str, ord: u16, question: bool) -> Cow<str> {
    let output = CLOZE.replace_all(text, |caps: &Captures| {
        let captured_ord = caps
            .get(cloze_caps::ORD)
            .unwrap()
            .as_str()
            .parse()
            .unwrap_or(0);

        if captured_ord != ord {
            // other cloze deletions are unchanged
            return caps.get(cloze_caps::TEXT).unwrap().as_str().to_owned();
        }

        let mut replacement;
        if question {
            // hint provided?
            if let Some(hint) = caps.get(cloze_caps::HINT) {
                replacement = format!("[{}]", hint.as_str());
            } else {
                replacement = "[...]".to_string()
            }
        } else {
            replacement = caps.get(cloze_caps::TEXT).unwrap().as_str().to_owned();
        }

        let can_use_html = caps.get(cloze_caps::C_CHAR).unwrap().as_str() == "c";
        if can_use_html {
            replacement = format!("<span class=cloze>{}</span>", replacement);
        }

        replacement
    });

    // if no cloze deletions are found, Anki returns an empty string
    match output {
        Cow::Borrowed(_) => "".into(),
        other => other,
    }
}

#[allow(dead_code)]
fn cloze_filter<'a>(text: &'a str, filter_args: &str, question: bool) -> Cow<'a, str> {
    let cloze_ord = filter_args.parse().unwrap_or(0);
    reveal_cloze_text(text, cloze_ord, question)
}

// Ruby filters
//----------------------------------------

lazy_static! {
    static ref FURIGANA: Regex = Regex::new(r" ?([^ >]+?)\[(.+?)\]").unwrap();
}

/// Did furigana regex match a sound tag?
fn captured_sound(caps: &Captures) -> bool {
    caps.get(2).unwrap().as_str().starts_with("sound:")
}

fn kana_filter(text: &str) -> Cow<str> {
    FURIGANA
        .replace_all(&text.replace("&nbsp;", " "), |caps: &Captures| {
            if captured_sound(caps) {
                caps.get(0).unwrap().as_str().to_owned()
            } else {
                caps.get(2).unwrap().as_str().to_owned()
            }
        })
        .into_owned()
        .into()
}

fn kanji_filter(text: &str) -> Cow<str> {
    FURIGANA
        .replace_all(&text.replace("&nbsp;", " "), |caps: &Captures| {
            if captured_sound(caps) {
                caps.get(0).unwrap().as_str().to_owned()
            } else {
                caps.get(1).unwrap().as_str().to_owned()
            }
        })
        .into_owned()
        .into()
}

fn furigana_filter(text: &str) -> Cow<str> {
    FURIGANA
        .replace_all(&text.replace("&nbsp;", " "), |caps: &Captures| {
            if captured_sound(caps) {
                caps.get(0).unwrap().as_str().to_owned()
            } else {
                format!(
                    "<ruby><rb>{}</rb><rt>{}</rt></ruby>",
                    caps.get(1).unwrap().as_str(),
                    caps.get(2).unwrap().as_str()
                )
            }
        })
        .into_owned()
        .into()
}

// Other filters
//----------------------------------------

/// convert to [[type:...]] for the gui code to process
fn type_filter<'a>(_text: &'a str, filter_args: &str, field_name: &str) -> Cow<'a, str> {
    if filter_args.is_empty() {
        format!("[[type:{}]]", field_name)
    } else {
        format!("[[type:{}:{}]]", filter_args, field_name)
    }
    .into()
}

// fixme: i18n
fn hint_filter<'a>(text: &'a str, field_name: &str) -> Cow<'a, str> {
    if text.trim().is_empty() {
        return text.into();
    }

    // generate a unique DOM id
    let mut hasher = Hasher::new();
    hasher.update(text.as_bytes());
    hasher.update(field_name.as_bytes());
    let id = hex::encode(&hasher.finalize().as_bytes()[0..8]);

    format!(
        r##"
<a class=hint href="#"
onclick="this.style.display='none';
document.getElementById('hint{}').style.display='block';
return false;">
{}</a>
<div id="hint{}" class=hint style="display: none">Show {}</div>
"##,
        id, text, id, field_name
    )
    .into()
}

// Tests
//----------------------------------------

#[cfg(test)]
mod test {
    use crate::template_filters::{
        apply_filters, cloze_filter, furigana_filter, hint_filter, kana_filter, kanji_filter,
        type_filter,
    };

    #[test]
    fn test_furigana() {
        let text = "test first[second] third[fourth]";
        assert_eq!(kana_filter(text).as_ref(), "testsecondfourth");
        assert_eq!(kanji_filter(text).as_ref(), "testfirstthird");
        assert_eq!(
            furigana_filter("first[second]").as_ref(),
            "<ruby><rb>first</rb><rt>second</rt></ruby>"
        );
    }

    #[test]
    fn test_hint() {
        assert_eq!(
            hint_filter("foo", "field"),
            r##"
<a class=hint href="#"
onclick="this.style.display='none';
document.getElementById('hint83fe48607f0f3a66').style.display='block';
return false;">
foo</a>
<div id="hint83fe48607f0f3a66" class=hint style="display: none">Show field</div>
"##
        );
    }

    #[test]
    fn test_type() {
        assert_eq!(type_filter("ignored", "", "Front"), "[[type:Front]]");
        assert_eq!(
            type_filter("ignored", "cloze", "Front"),
            "[[type:cloze:Front]]"
        );
        assert_eq!(
            apply_filters("ignored", &["type", "cloze"], "Text"),
            ("[[type:cloze:Text]]".into(), vec![])
        );
    }

    #[test]
    fn test_cloze() {
        let text = "{{C1::one}} {{C2::two::hint}}";
        assert_eq!(cloze_filter(text, "1", true), "[...] two");
        assert_eq!(cloze_filter(text, "2", true), "one [hint]");
        assert_eq!(cloze_filter(text, "1", false), "one two");
        assert_eq!(
            cloze_filter(&text.replace('C', "c"), "1", false),
            "<span class=cloze>one</span> two"
        );
        assert_eq!(
            cloze_filter(&text.replace('C', "c"), "1", true),
            "<span class=cloze>[...]</span> two"
        );
    }
}
