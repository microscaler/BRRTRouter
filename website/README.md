# Microscaler suite — brochure site (draft)

Static landing inspired by [rocket.rs](https://rocket.rs/), [loco.rs](https://loco.rs/),
and [actix.rs](https://actix.rs/): one composition, brand-first hero, code-forward
proof, three product pillars.

## Products

| Pillar | Repo | Pitch |
|--------|------|--------|
| **BRRTRouter** | [microscaler/BRRTRouter](https://github.com/microscaler/BRRTRouter) | OpenAPI-first HTTP on stackful coroutines |
| **Lifeguard** | sibling ORM crate / repo | Coroutine-native Postgres data layer |
| **Sesame** | [microscaler/sesame-idam](https://github.com/microscaler/sesame-idam) | Public IDAM reference built on the stack |

## Preview locally

No build step. From this directory:

```bash
# on ms02 (or any static server)
python3 -m http.server 4173
# open http://127.0.0.1:4173/
```

Or open `index.html` directly in a browser (fonts need network).

## Roadmap (conference-ready)

1. **Now (this folder)** — visual language, IA, hero + suite story, Sesame CTA
2. **Domain** — `microscaler.rs` / `brrtrouter.rs` (or docs subdomain) + GitHub Pages / Cloudflare Pages
3. **Content pass** — real Goose numbers, short film/GIF of codegen → `impl/`, conference talk abstract
4. **Extract** — optional move to a dedicated `microscaler/website` repo when Lifeguard + Sesame share ownership
5. **Docs deep-link** — mdBook or rustdoc badges from each product page

Do **not** market private Hauliage / PriceWhisperer as the open showcase.
Sesame is the cloneable reference.

## Design notes

- Display: **Fraunces** · UI: **Sora** · Code: **IBM Plex Mono**
- Palette: deep forest ink + warm brass accent (not purple-gradient AI defaults)
- Hero budget: brand, one headline, one sentence, CTA group, one dominant code plane
