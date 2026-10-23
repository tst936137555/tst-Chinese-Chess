Local changes against miniquad 0.4.11:

- Invalidate OpenGL binding caches when deleting textures/render attachments; retain the existing font-atlas regression coverage.
- Compile Android display replacement only on Android, where it is used.
- Remove unused rectangle calculations from Windows window positioning; `SWP_NOSIZE` preserves the previous behavior.
- Avoid Android-only unused IME locks and the mutable-static-reference warning in the one-shot iOS startup callback.

Retain the upstream MIT and Apache-2.0 licenses. Recheck these patches when upgrading miniquad.
