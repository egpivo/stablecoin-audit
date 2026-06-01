# Product screenshots (committed)

Small, versioned assets for **product docs** only.

| File | Use |
|------|-----|
| [`architecture_pipeline.svg`](architecture_pipeline.svg) | v0 stack diagram (`backend_architecture_v0.md`) |
| [`architecture_pipeline.png`](architecture_pipeline.png) | PNG export of the same (optional embed) |

## Blog & article figures

Medium/article assets, UI captures, GIFs, draw.io sources, and capture scripts are under **`.local/blog/figures/`** and **`.local/blog/scripts/`** (gitignored; avoids the 500 KB pre-commit limit).

Index: `.local/blog/figures/ARTICLE_FIGURES.md`

## Regenerate pipeline PNG

```bash
rsvg-convert -w 1200 docs/product/screenshots/architecture_pipeline.svg \
  -o docs/product/screenshots/architecture_pipeline.png
```
