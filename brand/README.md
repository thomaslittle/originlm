# OriginLM brand primitives

Edit `tokens.json`, then run `python scripts/generate_brand_tokens.py`.
The generated CSS is imported by the control center and site; GPUI uses the
generated Rust constants. Product-specific layout tokens remain with their
surfaces. These primitives are the common source for the graphite/amber brand,
text, evidence colors and core surfaces in both themes.

Logo and font assets retain their existing provenance and license files. The
native font registration uses the font's internal family name; CSS uses its
variable-font declaration. Neither changes the underlying Red Hat Display font.
