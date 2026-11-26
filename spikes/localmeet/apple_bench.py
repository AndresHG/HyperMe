"""Runs apple/applestt per language and scores it; CPU counted for applestt + localspeechrecognition."""
import json, subprocess, threading, time, sys
from pathlib import Path
import psutil, jiwer
sys.path.insert(0, str(Path(__file__).parent))
from textnorm import norm  # noqa: E402

B = Path(__file__).parent
S = Path(__file__).resolve().parents[1] / "flowbench" / "samples"
gold = json.load(open(S / "gold.json"))
(B / "out").mkdir(exist_ok=True)


def long_wav(lang):
    """All 25 clips of one language back to back, a meeting-length file (kept in out/)."""
    import numpy as np, soundfile as sf
    path = B / "out" / f"long-{lang}.wav"
    if not path.exists():
        a = np.concatenate([sf.read(S / f"{lang}-{i:02d}.wav", dtype="float32")[0] for i in range(1, 26)])
        sf.write(path, a, 24000, subtype="PCM_16")
    return path
NAMES = ("localspeechrecognition", "applestt")


def track(stop, acc):
    while not stop.is_set():
        for p in psutil.process_iter(["name"]):
            try:
                if p.info["name"] in NAMES:
                    t = p.cpu_times()
                    acc[p.pid] = (acc.get(p.pid, (None, 0))[0] or t.user + t.system, t.user + t.system)
            except psutil.Error:
                pass
        time.sleep(0.2)


def run(locale, files):
    acc, stop = {}, threading.Event()
    th = threading.Thread(target=track, args=(stop, acc)); th.start()
    time.sleep(0.5)
    out = subprocess.run([str(B / "apple/applestt"), locale, *map(str, files)], capture_output=True, text=True).stdout
    stop.set(); th.join()
    # first-seen baseline for the long-lived daemon; new processes start near 0
    cpu = sum(e - s for s, e in acc.values())
    return [json.loads(l) for l in out.splitlines() if l.startswith("{")], cpu


hyps, res = {}, {}
for lang, loc in (("en", "en-US"), ("es", "es-ES")):
    files = [S / f"{lang}-{i:02d}.wav" for i in range(1, 26)]
    rows, cpu = run(loc, files)
    for r in rows:
        hyps[r["file"][:-4]] = r["text"]
    sel = sorted(k for k in gold if k.startswith(lang))
    res[f"wer_{lang}"] = jiwer.wer([norm(gold[k]) for k in sel], [norm(hyps[k]) for k in sel])
    dur = sum(r["secs"] for r in rows)
    res[f"rtf_{lang}"] = sum(r["time"] for r in rows) / dur
    res[f"cpu_per_audio_s_{lang}"] = cpu / dur
    (row,), cpu = run(loc, [long_wav(lang)])
    ref = " ".join(norm(gold[k]) for k in sel)
    res[f"long_{lang}_wer"] = jiwer.wer(ref, norm(row["text"]))
    res[f"long_{lang}_rtf"] = row["time"] / row["secs"]
    res[f"long_{lang}_cpu_per_s"] = cpu / row["secs"]
json.dump(hyps, open(B / "out/apple.json", "w"), ensure_ascii=False, indent=1)
print("apple", json.dumps({k: round(v, 3) for k, v in res.items()}))
