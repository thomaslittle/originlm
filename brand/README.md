# OriginLM brand primitives

Edit `tokens.json`, then run `python scripts/generate_brand_tokens.py`.
The generated CSS is imported by the control center and site; GPUI uses the
generated Rust constants. Product-specific layout tokens remain with their
surfaces. These primitives are the common source for the graphite/amber brand,
text, evidence colors and core surfaces in both themes.

Logo and font assets retain their existing provenance and license files. The
native font registration uses the font's internal family name; CSS uses its
variable-font declaration. Neither changes the underlying Red Hat Display font.

## Pointer-following opening

The website header/footer and native in-app logos keep the supplied outer contour and outlined lettering fixed. Only the large O's inner opening moves, within a 12-by-10-unit ellipse in the 205-by-208 mark coordinate space. Movement eases to the pointer and returns to the exact supplied rest image on exit. Static favicon, launcher and exported artwork remain unchanged.

Website motion uses one shared pointer listener and direct SVG updates; native motion invalidates only small logo views. Both stop scheduling frames once settled and freeze at rest for reduced motion. The native app reads the OS preference at startup and honors `ORIGINLM_REDUCED_MOTION=1` (or `0` to override); changing an OS preference takes effect on the next launch. macOS and Windows preference adapters require platform verification.
