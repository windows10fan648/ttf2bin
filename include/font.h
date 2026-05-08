#ifndef FNT_FONT_H
#define FNT_FONT_H

/**
 * font.h — loader for .bin fonts and .fntpkg font systems produced by ttf2bin
 *
 * No dynamic allocation; everything points into the raw binary blob.
 * Works on any freestanding C environment (no libc required beyond stdint.h).
 *
 * ── Quick start ──────────────────────────────────────────────────────────
 *
 *  Single font (.bin):
 *    fnt_font_t font;
 *    fnt_load(&font, my_bin_blob);
 *    fnt_glyph_t g;
 *    fnt_get_glyph(&font, 'A', &g);
 *
 *  Font system (.fntpkg):
 *    fnt_pkg_t pkg;
 *    fnt_pkg_load(&pkg, my_fntpkg_blob);
 *    fnt_font_t font;
 *    fnt_pkg_find(&pkg, "Sans", "Bold", 16, &font);
 *    fnt_glyph_t g;
 *    fnt_get_glyph(&font, 'A', &g);
 */

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ── Return codes ────────────────────────────────────────────────────────── */
#define FNT_OK           0
#define FNT_ERR_MAGIC   -1   /* bad magic bytes          */
#define FNT_ERR_RANGE   -2   /* codepoint out of range   */
#define FNT_ERR_NOTFOUND -3  /* family/style/size absent */

/* ═══════════════════════════════════════════════════════════════════════════
 * Single-font API  (.bin)
 * ═══════════════════════════════════════════════════════════════════════════ */

/* ── Layout constants ────────────────────────────────────────────────────── */
#define FNT_BIN_MAGIC        "FNT\0"
#define FNT_BIN_HEADER_SIZE  16
#define FNT_BIN_INDEX_STRIDE 12

typedef struct {
    uint8_t  version;
    uint8_t  px_size;
    uint16_t first_char;
    uint16_t last_char;
    uint16_t glyph_count;

    /* internal */
    const uint8_t *_index;
    const uint8_t *_data;
} fnt_font_t;

typedef struct {
    const uint8_t *bitmap;   /* 8-bit alpha, row-major, top-to-bottom */
    uint8_t  width;
    uint8_t  height;
    int8_t   advance_x;      /* pixels to advance cursor horizontally */
    int8_t   bearing_x;      /* left bearing                          */
} fnt_glyph_t;

/**
 * fnt_load — initialise a font handle from a raw .bin blob.
 * Returns FNT_OK on success, FNT_ERR_MAGIC if the magic bytes don't match.
 */
static inline int fnt_load(fnt_font_t *font, const uint8_t *data)
{
    if (data[0] != 'F' || data[1] != 'N' || data[2] != 'T' || data[3] != '\0')
        return FNT_ERR_MAGIC;

    font->version     = data[4];
    font->px_size     = data[5];
    font->first_char  = (uint16_t)(data[6]  | ((uint16_t)data[7]  << 8));
    font->last_char   = (uint16_t)(data[8]  | ((uint16_t)data[9]  << 8));
    font->glyph_count = (uint16_t)(data[10] | ((uint16_t)data[11] << 8));
    font->_index      = data + FNT_BIN_HEADER_SIZE;
    font->_data       = font->_index + (uint32_t)font->glyph_count * FNT_BIN_INDEX_STRIDE;
    return FNT_OK;
}

/**
 * fnt_get_glyph — fill *out with glyph info for Unicode codepoint cp.
 * Returns FNT_OK on success, FNT_ERR_RANGE if cp is outside the font's range.
 */
static inline int fnt_get_glyph(const fnt_font_t *font, uint32_t cp, fnt_glyph_t *out)
{
    if (cp < font->first_char || cp > font->last_char)
        return FNT_ERR_RANGE;

    uint32_t        idx   = cp - font->first_char;
    const uint8_t  *entry = font->_index + idx * FNT_BIN_INDEX_STRIDE;
    uint32_t        off   = (uint32_t)(entry[0] | ((uint32_t)entry[1] << 8)
                                     | ((uint32_t)entry[2] << 16) | ((uint32_t)entry[3] << 24));

    out->width     = entry[8];
    out->height    = entry[9];
    out->advance_x = (int8_t)entry[10];
    out->bearing_x = (int8_t)entry[11];
    out->bitmap    = font->_data + off;
    return FNT_OK;
}

/**
 * fnt_measure_string — pixel width of a null-terminated ASCII/UTF-8 string.
 * Multi-byte codepoints are treated as single bytes for simplicity; extend
 * as needed for full UTF-8 decoding.
 */
static inline int fnt_measure_string(const fnt_font_t *font, const char *str)
{
    int width = 0;
    fnt_glyph_t g;
    while (*str) {
        if (fnt_get_glyph(font, (uint8_t)*str, &g) == FNT_OK)
            width += g.advance_x;
        str++;
    }
    return width;
}

/* ═══════════════════════════════════════════════════════════════════════════
 * Font system API  (.fntpkg)
 * ═══════════════════════════════════════════════════════════════════════════ */

/* ── Layout constants ────────────────────────────────────────────────────── */
#define FNT_PKG_MAGIC          "FPKG"
#define FNT_PKG_HEADER_SIZE    32
#define FNT_PKG_FAMILY_STRIDE  64
#define FNT_PKG_ENTRY_STRIDE   32

typedef struct {
    uint8_t  version;
    uint8_t  family_count;
    uint32_t entry_count;

    /* internal */
    const uint8_t *_family_dir;   /* points to Family Directory section */
    const uint8_t *_entry_table;  /* points to Entry Table section      */
    const uint8_t *_data;         /* points to Data section             */
} fnt_pkg_t;

/**
 * fnt_pkg_load — initialise a package handle from a raw .fntpkg blob.
 * Returns FNT_OK on success, FNT_ERR_MAGIC if the magic bytes don't match.
 */
static inline int fnt_pkg_load(fnt_pkg_t *pkg, const uint8_t *data)
{
    if (data[0] != 'F' || data[1] != 'P' || data[2] != 'K' || data[3] != 'G')
        return FNT_ERR_MAGIC;

    pkg->version      = data[4];
    pkg->family_count = data[7];
    pkg->entry_count  = (uint32_t)(data[8]  | ((uint32_t)data[9]  << 8)
                                 | ((uint32_t)data[10] << 16) | ((uint32_t)data[11] << 24));

    pkg->_family_dir  = data + FNT_PKG_HEADER_SIZE;
    pkg->_entry_table = pkg->_family_dir + (uint32_t)pkg->family_count * FNT_PKG_FAMILY_STRIDE;
    pkg->_data        = pkg->_entry_table + pkg->entry_count * FNT_PKG_ENTRY_STRIDE;
    return FNT_OK;
}

/* internal: compare a null-padded fixed-width field against a C string */
static inline int _fnt_field_eq(const uint8_t *field, int field_len, const char *str)
{
    int i = 0;
    while (i < field_len && str[i] != '\0') {
        if (field[i] != (uint8_t)str[i]) return 0;
        i++;
    }
    /* both must end at the same position */
    return (i == field_len || field[i] == '\0') && str[i] == '\0';
}

/**
 * fnt_pkg_find — locate a font by family name, style, and pixel size,
 * then initialise *font so it can be used with fnt_get_glyph().
 *
 * Returns FNT_OK on success, FNT_ERR_NOTFOUND if no match exists.
 *
 * Example:
 *   fnt_font_t font;
 *   fnt_pkg_find(&pkg, "Sans", "Bold", 16, &font);
 */
static inline int fnt_pkg_find(
    const fnt_pkg_t *pkg,
    const char      *family,
    const char      *style,
    uint8_t          px_size,
    fnt_font_t      *out_font)
{
    for (uint8_t fi = 0; fi < pkg->family_count; fi++) {
        const uint8_t *fam = pkg->_family_dir + (uint32_t)fi * FNT_PKG_FAMILY_STRIDE;

        if (!_fnt_field_eq(fam, 32, family))
            continue;

        uint32_t first_entry = (uint32_t)(fam[32] | ((uint32_t)fam[33] << 8)
                                        | ((uint32_t)fam[34] << 16) | ((uint32_t)fam[35] << 24));
        uint32_t entry_count = (uint32_t)(fam[36] | ((uint32_t)fam[37] << 8)
                                        | ((uint32_t)fam[38] << 16) | ((uint32_t)fam[39] << 24));

        for (uint32_t ei = 0; ei < entry_count; ei++) {
            const uint8_t *entry = pkg->_entry_table
                                 + (first_entry + ei) * FNT_PKG_ENTRY_STRIDE;

            if (!_fnt_field_eq(entry, 16, style))   continue;
            if (entry[16] != px_size)                continue;

            /* found — point the font handle at the embedded .bin blob */
            uint32_t offset = (uint32_t)(entry[17] | ((uint32_t)entry[18] << 8)
                                       | ((uint32_t)entry[19] << 16) | ((uint32_t)entry[20] << 24));
            return fnt_load(out_font, pkg->_data + offset);
        }
    }
    return FNT_ERR_NOTFOUND;
}

/**
 * fnt_pkg_list — iterate over every entry in the package.
 * Calls callback(family, style, px_size, userdata) for each entry.
 * Useful for building a font picker UI or debug listing.
 */
typedef void (*fnt_pkg_iter_fn)(
    const char *family, const char *style, uint8_t px_size, void *userdata);

static inline void fnt_pkg_list(const fnt_pkg_t *pkg, fnt_pkg_iter_fn cb, void *userdata)
{
    for (uint8_t fi = 0; fi < pkg->family_count; fi++) {
        const uint8_t *fam = pkg->_family_dir + (uint32_t)fi * FNT_PKG_FAMILY_STRIDE;

        uint32_t first_entry = (uint32_t)(fam[32] | ((uint32_t)fam[33] << 8)
                                        | ((uint32_t)fam[34] << 16) | ((uint32_t)fam[35] << 24));
        uint32_t entry_count = (uint32_t)(fam[36] | ((uint32_t)fam[37] << 8)
                                        | ((uint32_t)fam[38] << 16) | ((uint32_t)fam[39] << 24));

        for (uint32_t ei = 0; ei < entry_count; ei++) {
            const uint8_t *entry = pkg->_entry_table
                                 + (first_entry + ei) * FNT_PKG_ENTRY_STRIDE;
            cb((const char *)fam,   /* family name (null-padded field) */
               (const char *)entry, /* style  name (null-padded field) */
               entry[16],           /* px_size                         */
               userdata);
        }
    }
}

#ifdef __cplusplus
}
#endif

#endif /* FNT_FONT_H */
