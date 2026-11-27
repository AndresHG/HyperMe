# Microphone start latency (AC-2 preparation), 26 November 2026

Time from `Microphone::start` to the first 20 ms frame delivered to the state machine, measured
by `crates/hyperme-platform/examples/mic_latency.rs` (release build), 50 starts each, 100 ms
apart. Numbers only; no audio was kept.

- Machine: MacBook Pro (MacBookPro18,3), Apple M1 Pro, macOS 26.6.2
- Input: the built-in microphone, 48 kHz, 1 channel; output 24 kHz mono PCM16
- Network: not involved

| Mode | Median | 90th percentile | Max |
| --- | --- | --- | --- |
| Open the input stream on every start, close on stop (slice 6, replaced) | 113.2 ms | 120.2 ms | 297.6 ms |
| Build the stream once, then `play` on start and `pause` on stop (experiment) | 68.2 ms | 72.7 ms | 222.5 ms |
| The same, as shipped after the owner chose option B (second run, same day) | 67.9 ms | 79.5 ms | 224.8 ms |

The max is the first start of each run, which builds the stream.

After a fix so that a buffer overrun (which macOS reports when a stream starts) no longer forces a
rebuild, five more runs of the shipped mode gave medians of 46.7, 69.3, 67.3, 64.5 and 69.7 ms
and 90th percentiles of 52.4 to 74.4 ms. The first run is an outlier; the usual median is
about 65 to 70 ms.

The target in AC-2 is under 50 ms at the median and under 80 ms at the 90th percentile, from
key-down. Neither mode meets the median; the measurement also excludes the hotkey's own delay.
Each figure includes up to 20 ms to fill the first frame and one device buffer.

Speech in the first 70 to 120 ms after key-down is not captured in either mode. Meeting AC-2
needs the input stream running before key-down, which keeps the microphone in use (and the
macOS indicator lit) while HyperMe runs.

The owner chose option B, play and pause, on 26 November 2026. With it the median misses AC-2's
50 ms and the 90th percentile is just under its 80 ms. Whether to change AC-2 is for the spec's
owner.

While paused, Core Audio reports the input device as not running
(`kAudioDevicePropertyDeviceIsRunningSomewhere` is 0; it is 1 while recording), checked twice
in a row with a small probe. The menu-bar indicator is expected to follow device use; that is not
verified here, and the indicator was not checked by
eye, because screen capture was not available to the session.

## 27 November 2026: where the start time goes, and AC-2 at the first sample

- Machine: MacBook Pro (MacBookPro18,3), Apple M1 Pro, macOS 26.6.2
- Input: the built-in microphone, 48 kHz, 1 channel, default buffer (first callback 512 frames)
- Network: not involved

**Phase breakdown** (a probe, 30 play and pause starts per buffer size, milliseconds from the
device lookup, median / p90). `play()` returning takes about 45 ms: that is the macOS device
start. Smaller buffers do not help.

| Buffer | `play()` returns | First callback | 20 ms of audio |
| --- | --- | --- | --- |
| Default (512 frames) | 44.6 / 53.4 | 54.7 / 63.3 | 65.4 / 74.0 |
| 256 frames | 52.8 / 63.2 | 57.3 / 67.5 | 73.2 / 83.3 |
| 128 frames | 47.5 / 60.2 | 49.5 / 61.9 | 68.0 / 80.4 |

**#173**, starting the microphone before the app lookup and the history entry, took this off
the key-down path (50 runs): median 0.8 ms, p90 5.7 ms, max 27.5 ms.

**AC-2 at the first captured sample**, the measure the owner chose on 27 November 2026:
`crates/hyperme-platform/examples/mic_first_sample.rs` (release build), 50 starts of one built
stream, 150 ms apart, default buffer. The first sample is the first callback's arrival minus its
length.

| Run | `play()` returns (median / p90 / max) | First callback (median / p90 / max) | First captured sample (median / p90 / max) |
| --- | --- | --- | --- |
| 1 | 45.5 / 57.1 / 81.2 | 55.6 / 63.5 / 84.6 | 44.9 / 52.9 / 74.0 |
| 2 | 44.5 / 68.8 / 110.2 | 54.0 / 74.4 / 119.3 | 43.3 / 63.8 / 108.6 |
| 3 | 44.2 / 50.7 / 121.5 | 54.5 / 60.7 / 124.3 | 43.8 / 50.0 / 113.6 |

Verdict: **pass** in all three runs (median under 50 ms, p90 under 80 ms). The margin on the
median is about 5 ms. The figure excludes the hotkey's own delay.
