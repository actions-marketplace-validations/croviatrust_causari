# causari.dev

Static, zero-build, deployed to Cloudflare Pages from `site/` of this repo.
No framework, no bundler, no external fonts, no third-party requests on the
landing page (the measurements page fetches its data from this repo's
`leaderboard-data` branch and nothing else).

## Local preview

```sh
python3 -m http.server 8080 --directory site
```

## Files

| File | Purpose |
|---|---|
| `index.html` | landing: audit, ledger, proof, family, install |
| `survival.html` + `survival.js` + `survival-data.json` | weekly measurements (unranked; see `/method`) |
| `method.html` | how the numbers are made, what they cannot see, how to contest them |
| `styles.css` | the whole design system: four values (ink, paper, graphite, mist), system fonts |
| `app.js` | theme toggle, copy buttons, year; nothing else |
| `_headers` | security headers, CSP, caching |
| `_redirects` | vanity URLs and renamed asset paths |
| `assets/` | identity: `mark.svg`, `mark-white.svg`, `favicon.svg`, `wordmark*.svg`, `og.png` |
| `llms.txt`, `robots.txt`, `sitemap.xml` | machine readers |
| `install.sh`, `install.ps1` | installers, checksum-verified |

## Identity

The mark is `∵` ("because"): three discs, two causes above one effect. The
wordmark is `causari` in the system monospace. The palette is ink `#0b0d10`,
paper `#f5f4ef`, graphite `#3b4252`, mist `#9aa3ad`; dark and light are the
same four values swapped. Numbers never carry colour. The glyph set is
`∵` (cause), `⊢` (proves), `·`, `—`.

Master files live in the repo root `assets/`; `site/assets/` holds the copies
the site serves. Regenerate the PNGs and the copies with:

```sh
python3 tools/identity.py     # writes assets/*.svg, assets/*.png
cp assets/{mark,mark-white,favicon,wordmark,wordmark-white}.svg assets/og.png site/assets/
```

Asset filenames changed on 2026-09-20; the old paths redirect (see
`_redirects`) because `/assets/*` is served as immutable.
