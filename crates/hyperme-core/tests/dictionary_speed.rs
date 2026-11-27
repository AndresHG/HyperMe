//! Spec 93, AC-6 (R-6): substitution plus the choice of prompt entries, over 1,000 entries and
//! a 2,000-word transcript, takes under 1 ms in a release build. A debug build gets a generous
//! bound, so CI still catches a slow algorithm. Run the real check with
//! `cargo test --release -p hyperme-core --test dictionary_speed`.

use std::time::{Duration, Instant};

use hyperme_core::dictionary::{Dictionary, PROMPT_CAP};

const CONSONANTS: &[u8] = b"bcdfghjklmnpqrstvwxz";
const VOWELS: &[u8] = b"aeiou";

/// A small deterministic generator, so the test needs no crate and always sees the same text.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as usize) % n
    }

    /// An invented word of `parts` syllables: consonant, vowel and sometimes a consonant.
    fn word(&mut self, parts: usize) -> String {
        let mut word = String::new();
        for _ in 0..parts {
            word.push(char::from(CONSONANTS[self.next(CONSONANTS.len())]));
            word.push(char::from(VOWELS[self.next(VOWELS.len())]));
            if self.next(3) == 0 {
                word.push(char::from(CONSONANTS[self.next(CONSONANTS.len())]));
            }
        }
        word
    }
}

/// 1,000 invented names of three syllables, each with two heard forms of two words, as a
/// dictionary file.
fn dictionary(rng: &mut Lcg) -> Dictionary {
    let mut text = String::new();
    for i in 0..1000 {
        let name = rng.word(3);
        let mut written: Vec<char> = name.chars().collect();
        written[0] = written[0].to_ascii_uppercase();
        let written: String = written.into_iter().collect();
        text.push_str(&format!(
            "{written}{i}: {} {}, {} {i}\n",
            rng.word(2),
            rng.word(2),
            rng.word(3)
        ));
    }
    Dictionary::parse(&text)
}

/// 2,000 words: mostly ordinary words, a few that sound like names.
fn transcript(rng: &mut Lcg) -> String {
    const COMMON: &[&str] = &[
        "the", "report", "send", "and", "to", "team", "today", "que", "para", "el", "build",
        "meeting", "with", "about", "pero", "la", "review", "deploy", "mañana", "we",
    ];
    (0..2000)
        .map(|i| {
            if i % 25 == 0 {
                rng.word(2)
            } else {
                COMMON[rng.next(COMMON.len())].to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn substitution_and_prompt_choice_are_fast() {
    let mut rng = Lcg(93);
    let d = dictionary(&mut rng);
    assert_eq!(d.terms().len(), 1000);
    let text = transcript(&mut rng);
    assert_eq!(text.split_whitespace().count(), 2000);

    let mut times: Vec<Duration> = (0..9)
        .map(|_| {
            let start = Instant::now();
            let said = d.substitute(&text);
            let line = d.relevant_line(&said, PROMPT_CAP);
            let took = start.elapsed();
            assert!(line.is_some());
            took
        })
        .collect();
    times.sort();
    let median = times[times.len() / 2];
    let bound = if cfg!(debug_assertions) {
        Duration::from_millis(100)
    } else {
        Duration::from_millis(1)
    };
    eprintln!("substitute + relevant_line: median {median:?} (bound {bound:?})");
    assert!(median < bound, "median {median:?}, bound {bound:?}");
}
