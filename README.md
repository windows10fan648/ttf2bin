# ttf2bin — TTF font converter & font system builder

Converts TrueType fonts into compact binary formats ready to embed in any OS or bare-metal project.

Two output formats:
- **`.bin`** — a single rasterized font (one face, one size)
- **`.fntpkg`** — a font system package (multiple families, styles, and sizes in one file)

---

## Build

```bash
cargo build --release
# binary at: target/release/ttf2bin
```

---

## Commands

### `convert` — single font

```
ttf2bin convert [OPTIONS] --input <FILE> --output <FILE>

Options:
  -i, --input   <FILE>   Input .ttf font file
  -o, --output  <FILE>   Output .bin file
  -s, --size    <PX>     Rasterize size in pixels  [default: 16]
      --first   <CP>     First codepoint (decimal) [default: 32  = space]
      --last    <CP>     Last  codepoint (decimal) [default: 126 = ~]
```

```bash
ttf2bin convert -i fonts/MyFont.ttf -o MyFont_16.bin -s 16
ttf2bin convert -i fonts/MyFont.ttf -o MyFont_32.bin -s 32 --first 32 --last 255
```

---

### `system` — font system package

```
ttf2bin system --manifest <TOML> --output <FILE>

Options:
  -m, --manifest  <FILE>   Font system manifest (.toml)
  -o, --output    <FILE>   Output .fntpkg file
```

```bash
ttf2bin system --manifest example/fonts.toml --output system.fntpkg
```

The manifest declares font families, faces, and sizes.  See `example/fonts.toml`:

```toml
[system]
name    = "MyOS Font System"
version = "1.0.0"

[[family]]
name = "Sans"

  [[family.face]]
  style = "Regular"
  ttf   = "fonts/Sans-Regular.ttf"
  sizes = [12, 16, 24]

  [[family.face]]
  style = "Bold"
  ttf   = "fonts/Sans-Bold.ttf"
  sizes = [12, 16, 24]

[[family]]
name = "Mono"

  [[family.face]]
  style = "Regular"
  ttf   = "fonts/Mono-Regular.ttf"
  sizes = [12, 14, 16]
```

---

## Binary formats

### `.bin` — single font

```
Offset  Size  Field
──────────────────────────────────────────────────────
Header (16 bytes)
  0       4   magic        "FNT\0"
  4       1   version      1
  5       1   px_size
  6       2   first_char   LE u16
  8       2   last_char    LE u16
 10       2   glyph_count  LE u16
 12       4   reserved

Glyph Index  (glyph_count × 12 bytes)
  +0      4   data_offset  LE u32  — offset into Data section
  +4      4   data_size    LE u32
  +8      1   width
  +9      1   height
 +10      1   advance_x    signed
 +11      1   bearing_x    signed

Data section
  8-bit alpha bitmaps, row-major, top-to-bottom.
  0 = transparent, 255 = fully opaque.
```

### `.fntpkg` — font system package

```
Offset  Size  Field
──────────────────────────────────────────────────────
PKG Header (32 bytes)
  0       4   magic          "FPKG"
  4       1   version        1
  5       2   reserved
  7       1   family_count
  8       4   entry_count    LE u32
 12      20   reserved

Family Directory  (family_count × 64 bytes)
  +0     32   name           null-padded UTF-8
 +32      4   first_entry    LE u32  — index into Entry Table
 +36      4   entry_count    LE u32
 +40     24   reserved

Entry Table  (entry_count × 32 bytes)
  +0     16   style          null-padded UTF-8
 +16      1   px_size
 +17      4   data_offset    LE u32  — offset into Data section
 +21      4   data_size      LE u32
 +25      7   reserved

Data section
  Concatenated .bin font blobs (each self-describing via FNT\0 header).
```

---

## Using fonts in your OS (C)

Include `include/font.h` — no libc required, no heap allocation.

### Single font

```c
#include "font.h"

uint8_t *blob = load_file("MyFont_16.bin");  // map however your OS does it

fnt_font_t font;
fnt_load(&font, blob);

// render 'A' at (x, y)
fnt_glyph_t g;
fnt_get_glyph(&font, 'A', &g);
for (int row = 0; row < g.height; row++)
    for (int col = 0; col < g.width; col++) {
        uint8_t alpha = g.bitmap[row * g.width + col];
        if (alpha) put_pixel(x + col, y + row, blend(fg, bg, alpha));
    }
x += g.advance_x;
```

### Font system

```c
#include "font.h"

uint8_t *blob = load_file("system.fntpkg");

fnt_pkg_t pkg;
fnt_pkg_load(&pkg, blob);

// look up Sans Bold 16px
fnt_font_t font;
if (fnt_pkg_find(&pkg, "Sans", "Bold", 16, &font) == FNT_OK) {
    fnt_glyph_t g;
    fnt_get_glyph(&font, 'H', &g);
    // ... render ...
}

// list all available fonts
void print_font(const char *fam, const char *style, uint8_t sz, void *ud) {
    printf("%s / %s @ %upx\n", fam, style, sz);
}
fnt_pkg_list(&pkg, print_font, NULL);
```
