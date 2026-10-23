# Local audio mixer repair

Upstream: not-fl3/quad-snd, version 0.2.8 (MIT / Apache-2.0).

When a looping sound ended partway through an output buffer, the original mixer
mixed the next loop at offset zero, overwriting the tail and leaving silence at
the end. The local change writes at the current buffer offset. Empty sounds are
also ignored so that an empty loop cannot spin forever in the audio callback.
The final mix has a smooth limiter above 0.95 to prevent overlapping cues from clipping.

No platform backend or public API was changed. See tests in `src/mixer.rs`.
