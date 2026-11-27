//! The Dictionary hand-off between Home (#98) and the personal dictionary (#93): the shape of
//! a correction and the common-word confirm round trip. The dictionary itself lives in #93.

mod common;

use common::{FakeDictionary, err};
use hyperme_core::traits::{Added, Correction, Dictionary, ErrorKind, Refusal};

fn correction(confirmed_common: bool) -> Correction {
    Correction {
        heard: "cloud".to_owned(),
        wanted: "Claude".to_owned(),
        dictation: Some(7),
        language: Some("en".to_owned()),
        confirmed_common,
    }
}

/// Sends a correction as Home's Fix a word does (#98 R-17): a common-word warning is shown,
/// and the correction sent again with `confirmed_common` only if the user confirms.
fn fix_a_word(dict: &mut impl Dictionary, user_confirms: bool) -> Result<Added, String> {
    match dict.add_correction(correction(false)) {
        Ok(Added::NeedsConfirm(Refusal::CommonWord)) if user_confirms => {
            dict.add_correction(correction(true)).map_err(|e| e.message)
        }
        other => other.map_err(|e| e.message),
    }
}

/// #98 AC-11 (core half, R-17, R-20): `NeedsConfirm` leads to a second call with
/// `confirmed_common = true`, carrying the exact same selection and word.
#[test]
fn common_word_needs_confirm_then_is_sent_again() {
    let mut dict = FakeDictionary::default();
    dict.answers
        .push_back(Ok(Added::NeedsConfirm(Refusal::CommonWord)));
    assert_eq!(fix_a_word(&mut dict, true), Ok(Added::Added));
    assert_eq!(dict.corrections, [correction(false), correction(true)]);
}

/// #98 AC-11 (core half): without the user's confirmation nothing is sent again.
#[test]
fn common_word_not_confirmed_is_not_sent_again() {
    let mut dict = FakeDictionary::default();
    dict.answers
        .push_back(Ok(Added::NeedsConfirm(Refusal::CommonWord)));
    assert_eq!(
        fix_a_word(&mut dict, false),
        Ok(Added::NeedsConfirm(Refusal::CommonWord))
    );
    assert_eq!(dict.corrections, [correction(false)]);
}

/// #98 AC-11 (core half, R-20): a hard refusal is an `Err` with a message to show, and is not
/// retried.
#[test]
fn hard_refusal_is_an_error_with_a_message() {
    let mut dict = FakeDictionary::default();
    dict.answers.push_back(Err(err(
        ErrorKind::Other,
        "That correction is already in the dictionary.",
    )));
    assert_eq!(
        fix_a_word(&mut dict, true),
        Err("That correction is already in the dictionary.".to_owned())
    );
    assert_eq!(dict.corrections.len(), 1);
}

/// #98 R-20: a plain correction is added at once, and the types compare and clone.
#[test]
fn plain_correction_is_added() {
    let mut dict = FakeDictionary::default();
    let c = correction(false);
    assert_eq!(dict.add_correction(c.clone()), Ok(Added::Added));
    assert_eq!(dict.corrections, [c]);
    assert_ne!(Added::Added, Added::NeedsConfirm(Refusal::CommonWord));
    let _ = format!("{:?}", Refusal::CommonWord);
}
