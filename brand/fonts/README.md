# Red Hat Display web font

`RedHatDisplay-Variable.woff2` preserves the complete variable font from
`apps/demo/src/assets/fonts/RedHatDisplay-Variable.ttf`, converted to WOFF2 with
FontTools 4.68.0. No glyphs or axes were removed. Both web applications consume this
one asset; GPUI uses the original TTF. The font remains under the SIL Open Font
License in `OFL.txt`, from the [Red Hat font project](https://github.com/RedHatOfficial/RedHatFont).

Reproduce with `TTFont(source)`, set `font.flavor = 'woff2'`, then `font.save(target)`
using `fonttools[woff]`. Conversion is a development operation, not a runtime dependency.
