# Local reliability patch

Upstream: miniquad 0.4.11, MIT OR Apache-2.0.

`GlContext::delete_texture` clears cached GL bindings before deleting a texture. OpenGL implicitly unbinds deleted textures, and native names can be reused. Retaining the deleted name in the cache can skip a necessary bind and turn font quads into opaque black rectangles, particularly after atlas or render-target replacement.

All other rendering behavior is retained. Application-level glyph preparation submits geometry before a font atlas can grow.
