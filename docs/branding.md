# PortSpy brand

PortSpy is a local tool for developers who need to identify what's listening on
their machine. Use the tagline **Know what's using your localhost.** Keep
product copy specific to listeners, processes, services and actions.

The mark combines the colon from a port address with a terminal chevron.
The terminal draws the two dots and chevron across three rows when room
allows. The compact form is `:>`, followed by the name `PortSpy`.

| Asset | Use |
| --- | --- |
| [SVG logo](../assets/brand/portspy-logo.svg) | Transparent master used in the README. The mark adapts to the color scheme. |
| [Transparent PNG logo](../assets/brand/portspy-logo.png) | 512 × 512 mint export of the SVG for dark surfaces and tools that accept PNG. |
| [Banner](../assets/brand/portspy-banner.png) | Wide graphite artwork for presentations and announcements. |
| [Square mark](../assets/brand/portspy-mark.png) | Project avatar and other square placements. Keep the built-in margins so it also fits a round crop. |

Use mint `#50F0C3` on graphite `#10161C` for the mark and blue-gray
`#26343E` for the banner texture. The SVG fallback uses `#16856D`. The TUI
uses graphite surfaces and a mint mark. Protocols and exposure have text
labels alongside their visual treatments. Keep the name visible wherever
the audience may be seeing the mark for the first time.

The README and terminal treatments follow the readability principles in the
[GitHub 2026 Brand Guidelines](https://brand.github.com/GitHub-BrandGuidelines-2026.pdf)
and [W3C accessibility guidance](https://www.w3.org/WAI/tips/designing/).

## Image generation

The banner and square mark were created with built-in ImageGen. The square
mark used the banner as a visual reference. The transparent ImageGen study
retained faint pixels between the shapes. The final PNG was exported from
the hand-traced SVG to give both logo formats the same clean silhouette.

**Banner prompt**

```text
Use case: logo-brand. Asset type: panoramic GitHub README brand banner for PortSpy, a local TCP/UDP listener inspector for developers. Create an original, quietly confident identity. Feature one simple, bold mint-green terminal mark centered on the horizontal axis: two small stacked square dots (a port-number colon) immediately followed by a single crisp right-pointing chevron made of two strokes. The silhouette should read as ':>' without forming other letters. Put it within a restrained horizontal field of graphite diagnostic-tool texture: sparse fine signal traces on a precise grid, slightly brighter around the mark and quickly falling away. Keep that texture subordinate and most of the canvas dark. Near-black navy graphite background #10161C, vivid cool mint #50F0C3 for the mark, muted blue-gray #26343E for texture. Flat 2D geometric strokes, editorial precision, clean edges, no glow. Extra-wide banner composition; keep all important content in the middle 40 percent of the image height with ample breathing room above and below. The horizontal edges should quietly fade into near-solid graphite. No typography whatsoever, including brand name, captions or digits. No fake dashboard, no LEDs, no hacker imagery, no shield, no magnifier, no watermark.
```

**Square mark prompt**

```text
Use case: logo-brand. Asset type: square repository avatar for PortSpy. Input image 1 is a brand reference. Reuse its exact central terminal symbol, two small stacked solid squares immediately followed by a single bold right-pointing chevron, as a flat mint #50F0C3 icon on one completely solid opaque graphite #10161C square canvas. Preserve the recognizable silhouette and the gap between the colon and chevron. Center it optically and make the complete mark occupy about 60 percent of the square's width, with generous margins for a round crop. Crisp flat color, tiny-size legibility at 24px. No textures, grid, gradients, border, corner rounding, effects, other symbols, numerals, lettering or typography.
```

**Transparent study prompt**

```text
Use case: background-extraction. Input image 1 is the edit target, an existing PortSpy mark with two stacked mint square dots and a mint chevron. Remove only the dark graphite background and replace it with genuinely transparent pixels (PNG alpha). Preserve the exact three filled shapes, square corners, mint color, proportions, positioning, spacing and existing square canvas. Keep the same clear margins. Flat clean opaque mint shapes and crisp antialiased edges. No outlines, shadow, effects, text, additional elements, or checkerboard baked into the pixels.
```
