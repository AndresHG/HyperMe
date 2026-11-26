"""Local speech-to-text benchmark on the flowbench samples. Prints numbers; transcripts go to out/ (git-ignored)."""
import json, resource, sys, time
from pathlib import Path
import numpy as np, soundfile as sf
from scipy.signal import resample_poly
import jiwer
from textnorm import norm

SAMPLES = Path(__file__).resolve().parents[1] / "flowbench" / "samples"
OUT = Path(__file__).parent / "out"
OUT.mkdir(exist_ok=True)
gold = json.load(open(SAMPLES / "gold.json"))
ids = sorted(gold)


def load16(i):
    a, sr = sf.read(SAMPLES / f"{i}.wav", dtype="float32")
    if a.ndim > 1:
        a = a.mean(1)
    return resample_poly(a, 2, 3).astype(np.float32) if sr == 24000 else a  # 24k -> 16k


audio = {i: load16(i) for i in ids}


def cpu():
    r = resource.getrusage(resource.RUSAGE_SELF)
    return r.ru_utime + r.ru_stime


def run(name, fn, warm):
    fn(warm)  # warm-up / load
    rows, hyps = [], {}
    for i in ids:
        a = audio[i]
        c0, t0 = cpu(), time.perf_counter()
        h = fn(a)
        dt, dc = time.perf_counter() - t0, cpu() - c0
        hyps[i] = h
        rows.append((i, len(a) / 16000, dt, dc))
    json.dump(hyps, open(OUT / f"{name}.json", "w"), ensure_ascii=False, indent=1)
    res = {}
    for lang in ("en", "es"):
        sel = [i for i in ids if i.startswith(lang)]
        res[f"wer_{lang}"] = jiwer.wer([norm(gold[i]) for i in sel], [norm(hyps[i]) for i in sel])
    dur = sum(r[1] for r in rows)
    res["rtf"] = sum(r[2] for r in rows) / dur
    res["cpu_per_audio_s"] = sum(r[3] for r in rows) / dur
    res["median_ms"] = float(np.median([r[2] for r in rows]) * 1000)
    res["audio_min"] = dur / 60
    # long-form: all ES then all EN concatenated (meeting-like length)
    for lang in ("es", "en"):
        long = np.concatenate([audio[i] for i in ids if i.startswith(lang)])
        c0, t0 = cpu(), time.perf_counter()
        h = fn(long)
        dt, dc = time.perf_counter() - t0, cpu() - c0
        ref = " ".join(norm(gold[i]) for i in ids if i.startswith(lang))
        res[f"long_{lang}_wer"] = jiwer.wer(ref, norm(h))
        res[f"long_{lang}_rtf"] = dt / (len(long) / 16000)
        res[f"long_{lang}_cpu_per_s"] = dc / (len(long) / 16000)
        res[f"long_{lang}_min"] = len(long) / 16000 / 60
    res["peak_rss_gb"] = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1e9
    print(name, json.dumps({k: round(v, 3) for k, v in res.items()}), flush=True)
    json.dump(res, open(OUT / f"{name}.metrics.json", "w"), indent=1)


which = sys.argv[1]
warm = audio[ids[0]]

if which.startswith("whisper"):
    import mlx_whisper
    repo = {"whisper-turbo": "mlx-community/whisper-large-v3-turbo",
            "whisper-small": "mlx-community/whisper-small-mlx"}[which]
    run(which, lambda a: mlx_whisper.transcribe(a, path_or_hf_repo=repo, verbose=None,
                                                condition_on_previous_text=False)["text"], warm)
elif which == "parakeet":
    import mlx.core as mx
    from parakeet_mlx import from_pretrained
    from parakeet_mlx.audio import get_logmel
    m = from_pretrained("mlx-community/parakeet-tdt-0.6b-v3")
    pc = m.preprocessor_config

    def tr(a):
        x = mx.array(a)
        if len(a) > 16000 * 120:  # chunk long audio like transcribe(chunk_duration=120)
            out, step = [], 16000 * 120
            for s in range(0, len(a), step):
                out.append(m.generate(get_logmel(x[s:s + step], pc))[0].text)
            return " ".join(out)
        return m.generate(get_logmel(x, pc))[0].text
    run(which, tr, warm)
elif which == "parakeet-stream":
    # live "gray" mode: feed 1 s chunks, measure per-chunk latency on long ES audio
    import mlx.core as mx
    from parakeet_mlx import from_pretrained
    m = from_pretrained("mlx-community/parakeet-tdt-0.6b-v3")
    for lang in ("es", "en"):
        long = np.concatenate([audio[i] for i in ids if i.startswith(lang)])
        lat = []
        c0, t0 = cpu(), time.perf_counter()
        with m.transcribe_stream(context_size=(256, 256)) as s:
            for k in range(0, len(long), 16000):
                t1 = time.perf_counter()
                s.add_audio(mx.array(long[k:k + 16000]))
                _ = s.result.text
                lat.append(time.perf_counter() - t1)
            text = s.result.text
        dt, dc = time.perf_counter() - t0, cpu() - c0
        ref = " ".join(norm(gold[i]) for i in ids if i.startswith(lang))
        sec = len(long) / 16000
        print("parakeet-stream", lang, json.dumps({
            "wer": round(jiwer.wer(ref, norm(text)), 3), "rtf": round(dt / sec, 3),
            "cpu_per_audio_s": round(dc / sec, 3),
            "chunk_ms_p50": round(float(np.median(lat)) * 1000, 1),
            "chunk_ms_p95": round(float(np.percentile(lat, 95)) * 1000, 1),
            "minutes": round(sec / 60, 2)}), flush=True)
        json.dump({"text": text}, open(OUT / f"stream-{lang}.json", "w"), ensure_ascii=False)
