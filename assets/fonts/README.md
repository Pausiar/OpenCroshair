# Bundled fonts

| File | Family | Version | Source | License |
|---|---|---|---|---|
| inter-regular.ttf | Inter Regular | 4.0 (subset) | https://github.com/rsms/inter/releases/download/v4.0/Inter-4.0.zip (extras/ttf/Inter-Regular.ttf) | SIL OFL 1.1 (OFL-Inter.txt) |
| inter-semibold.ttf | Inter SemiBold | 4.0 (subset) | same archive (extras/ttf/Inter-SemiBold.ttf) | SIL OFL 1.1 (OFL-Inter.txt) |
| source-serif-4-semibold.ttf | Source Serif 4 SemiBold | 4.005 (subset) | https://github.com/adobe-fonts/source-serif/releases/download/4.005R/source-serif-4.005_Desktop.zip (TTF/SourceSerif4-Semibold.ttf) | SIL OFL 1.1 (OFL-SourceSerif4.txt) |

All files were subset with `pyftsubset` (fontTools) to Basic Latin, Latin-1 Supplement, Latin Extended-A,
General Punctuation, the euro sign, trademark sign and the basic arrows; OpenType layout features kern, liga,
calt, ccmp, locl, mark and mkmk were kept. Glyph outlines and metrics are otherwise unmodified.
