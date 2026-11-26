"""Local LLM post-processing bench (mlx-lm). usage: llm_bench.py <hf-repo> <stt-name>"""
import json, resource, sys, time
from pathlib import Path
from mlx_lm import load, generate
from mlx_lm.sample_utils import make_sampler

B = Path(__file__).parent
S = Path(__file__).resolve().parents[1] / "flowbench" / "samples"
repo, stt = sys.argv[1], sys.argv[2]
gold = json.load(open(S / "gold.json"))
raw = json.load(open(B / "out" / f"{stt}.json"))
vocab = ", ".join(l.split(":")[0].strip() for l in open(S / "vocab.txt") if l.strip())
PROMPT = ("You clean up dictated text in Spanish or English. Change as little as possible. Remove only "
          "hesitations (um, uh, eh, em, mm), stutters and false starts; when the speaker corrects themselves, "
          "keep only the correction. Fix punctuation, capitals and accents, and words that were clearly misheard. "
          "Keep every other word exactly as spoken, including informal words, slang and swearing, in the same order "
          "and the same language. Never translate, rephrase, summarise or answer. Output only the cleaned text. "
          "When a word or short phrase clearly sounds like a term in the speaker's dictionary, write the dictionary "
          f"term. Otherwise change nothing.\nDictionary: {vocab}")


def lev(a, b):
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]


def nrm(t):
    return " ".join(t.split())


def cpu():
    r = resource.getrusage(resource.RUSAGE_SELF)
    return r.ru_utime + r.ru_stime


model, tok = load(repo)
greedy = make_sampler(temp=0.0)


def chat(system, user, max_tokens):
    msgs = [{"role": "system", "content": system}, {"role": "user", "content": user}]
    kw = {"enable_thinking": False} if "Qwen3-1.7B" in repo else {}
    p = tok.apply_chat_template(msgs, add_generation_prompt=True, tokenize=False, **kw)
    t0 = time.perf_counter()
    out = generate(model, tok, p, max_tokens=max_tokens, sampler=greedy)
    return out.strip(), time.perf_counter() - t0, len(tok.encode(p)), len(tok.encode(out))


chat(PROMPT, "hola", 8)  # warm-up
ids = sorted(gold)
edits_raw = edits_clean = chars = zero_raw = zero_clean = 0
lat, outtok, c0, wall0 = [], 0, cpu(), time.perf_counter()
cleaned = {}
for i in ids:
    out, dt, _, n = chat(PROMPT, raw[i], 400)
    cleaned[i] = out
    lat.append(dt); outtok += n
    g = nrm(gold[i])
    er, ec = lev(nrm(raw[i]), g), lev(nrm(out), g)
    edits_raw += er; edits_clean += ec; chars += len(g)
    zero_raw += er == 0; zero_clean += ec == 0
wall, dc = time.perf_counter() - wall0, cpu() - c0
lat.sort()
res = {"model": repo, "stt": stt,
       "edits_per_100_raw": 100 * edits_raw / chars, "edits_per_100_clean": 100 * edits_clean / chars,
       "zero_edit_raw": zero_raw, "zero_edit_clean": zero_clean, "n": len(ids),
       "clean_ms_p50": lat[len(lat) // 2] * 1000, "clean_ms_p90": lat[int(len(lat) * .9)] * 1000,
       "out_tok_per_s": outtok / wall, "cpu_s_per_sample": dc / len(ids)}
# meeting summary over the long Spanish raw transcript (~4 min)
long_raw = " ".join(raw[i] for i in ids if i.startswith("es"))
summ, dt, pin, pout = chat("Summarise this meeting transcript in Spanish: 5 bullet points, then a list of "
                           "action items with owner if stated. Be faithful; do not invent.", long_raw, 600)
res.update({"summary_s": dt, "summary_in_tok": pin, "summary_out_tok": pout,
            "peak_rss_gb": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1e9})
name = repo.split("/")[-1]
json.dump({"cleaned": cleaned, "summary": summ}, open(B / "out" / f"llm-{name}.json", "w"), ensure_ascii=False, indent=1)
print(json.dumps({k: (round(v, 2) if isinstance(v, float) else v) for k, v in res.items()}), flush=True)
