# Local audio mixer repair

Upstream: not-fl3/quad-snd, version 0.2.8 (MIT / Apache-2.0).

When a looping sound ended partway through an output buffer, the original mixer
mixed the next loop at offset zero, overwriting the tail and leaving silence at
the end. The local change writes at the current buffer offset. Empty sounds are
also ignored so that an empty loop cannot spin forever in the audio callback.
The final mix has a smooth limiter above 0.95 to prevent overlapping cues from clipping.

Native audio now has one FIFO decoder worker per mixer. File decoding, resampling
and conversion to shared sample storage happen on that worker, outside both the
UI thread and the real-time audio callback. Load returns after copying/enqueuing
the bytes; playback starts when the preceding load finishes. Play, volume, stop
and delete commands retain their order, including when queued during decoding.
Invalid audio installs an empty sound without terminating the worker. Closing the
control channel terminates the worker after queued commands have been processed.

No platform backend or public API signatures were changed. The WebAudio backend
is unaffected. Tests in `src/mixer.rs` cover load/play ordering without a running
decoder, stop/delete during loading, malformed input, looping and limiting.
