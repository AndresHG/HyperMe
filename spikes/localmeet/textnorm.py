"""Text normalisation for word error rate: lower case, punctuation dropped."""
import re
import unicodedata


def norm(t):
    t = unicodedata.normalize("NFC", t.lower())
    t = re.sub(r"[^\w\s']", " ", t)
    return re.sub(r"\s+", " ", t).strip()
