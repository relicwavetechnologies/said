//! Script utilities for enforcing output-language contracts.

pub fn contains_devanagari(text: &str) -> bool {
    text.chars().any(is_devanagari)
}

fn is_devanagari(ch: char) -> bool {
    ('\u{0900}'..='\u{097F}').contains(&ch)
}

fn independent_vowel(ch: char) -> Option<&'static str> {
    Some(match ch {
        'अ' => "a",
        'आ' => "aa",
        'इ' => "i",
        'ई' => "ee",
        'उ' => "u",
        'ऊ' => "oo",
        'ए' => "e",
        'ऐ' => "ai",
        'ओ' => "o",
        'औ' => "au",
        'ऋ' => "ri",
        _ => return None,
    })
}

fn consonant(ch: char) -> Option<&'static str> {
    Some(match ch {
        'क' => "k",
        'ख' => "kh",
        'ग' => "g",
        'घ' => "gh",
        'ङ' => "ng",
        'च' => "ch",
        'छ' => "chh",
        'ज' => "j",
        'झ' => "jh",
        'ञ' => "ny",
        'ट' => "t",
        'ठ' => "th",
        'ड' => "d",
        'ढ' => "dh",
        'ण' => "n",
        'त' => "t",
        'थ' => "th",
        'द' => "d",
        'ध' => "dh",
        'न' => "n",
        'प' => "p",
        'फ' => "ph",
        'ब' => "b",
        'भ' => "bh",
        'म' => "m",
        'य' => "y",
        'र' => "r",
        'ल' => "l",
        'व' => "v",
        'श' => "sh",
        'ष' => "sh",
        'स' => "s",
        'ह' => "h",
        'क़' => "q",
        'ख़' => "kh",
        'ग़' => "gh",
        'ज़' => "z",
        'ड़' => "d",
        'ढ़' => "dh",
        'फ़' => "f",
        _ => return None,
    })
}

fn matra(ch: char) -> Option<&'static str> {
    Some(match ch {
        'ा' => "aa",
        'ि' => "i",
        'ी' => "ee",
        'ु' => "u",
        'ू' => "oo",
        'ृ' => "ri",
        'े' => "e",
        'ै' => "ai",
        'ो' => "o",
        'ौ' => "au",
        _ => return None,
    })
}

fn diacritic(ch: char) -> Option<&'static str> {
    Some(match ch {
        'ं' | 'ँ' => "n",
        'ः' => "h",
        '़' => "",
        'ऽ' => "",
        _ => return None,
    })
}

/// Romanize Devanagari into readable Hinglish while leaving existing Roman
/// English/Hinglish untouched. This is a deterministic guardrail, not a full
/// linguistic transliterator; the LLM prompt still carries the primary style
/// instruction.
pub fn romanize_devanagari(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut it = text.chars().peekable();

    while let Some(ch) = it.next() {
        if let Some(v) = independent_vowel(ch) {
            out.push_str(v);
            continue;
        }

        if let Some(base) = consonant(ch) {
            out.push_str(base);
            match it.peek().copied() {
                Some(next) if matra(next).is_some() => {
                    out.push_str(matra(next).unwrap_or_default());
                    it.next();
                }
                Some('्') => {
                    it.next();
                }
                _ => out.push('a'),
            }
            continue;
        }

        if let Some(v) = matra(ch).or_else(|| diacritic(ch)) {
            out.push_str(v);
            continue;
        }

        if ch == '्' {
            continue;
        }

        out.push(ch);
    }

    out
}

pub fn enforce_roman_hinglish(text: &str) -> String {
    if contains_devanagari(text) {
        romanize_devanagari(text)
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn romanizes_common_hindi_to_hinglish_script() {
        let out = enforce_roman_hinglish("आज बहुत काम था, मैं थक गया हूँ.");
        assert!(!contains_devanagari(&out));
        assert!(out.contains("aaj"));
        assert!(out.contains("main"));
    }

    #[test]
    fn leaves_roman_text_unchanged() {
        let text = "Aaj bahut kaam tha, but deployment went fine.";
        assert_eq!(enforce_roman_hinglish(text), text);
    }
}
