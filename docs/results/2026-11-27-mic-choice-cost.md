# Cost of choosing a microphone on key-down (#221)

Date: 27 November 2026. Machine: the owner's MacBook Pro (Apple silicon), macOS, release build,
20 runs each, five input devices connected (built-in, a Continuity iPhone and three virtual
devices). Network: not involved.

| Call | Median | Max |
| --- | --- | --- |
| `default_input_device()` and its id | 18 µs | 89.2 ms (first call, Core Audio cold) |
| `input_devices()` with every device's name | 2.3 ms | 8.5 ms |

What follows from it:

- With no preferred microphone, start reads only the default, as before #221: no added cost.
- With a preference, a stream already on the first preferred microphone is reused without
  listing the devices. Only a dictation on a lower choice lists them, to see whether a better
  microphone came back: 2 to 8 ms of the 50 ms key-down budget.
- The cold first call (about 89 ms) predates #221 and falls on the first dictation after launch.
  HyperMe now warms the host on a short-lived thread at launch (#250).

`START_SOUND`: `afinfo /System/Library/Sounds/Tink.aiff` gives an estimated duration of
0.564 s. The mute waits 250 ms, past the click.
