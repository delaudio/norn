# Norn Web

Minimal Astro website for Norn.

## Development

Run the site from the repository root:

```sh
pnpm web:dev
```

The app lives in `apps/web` and builds independently from the desktop app:

```sh
pnpm web:build
```

Astro writes the static site to `apps/web/dist`.

## Deployment

Use these settings for static hosts:

| Host | Build command | Publish directory |
| --- | --- | --- |
| GitHub Pages | `pnpm install --frozen-lockfile && pnpm web:build` | `apps/web/dist` |
| Netlify | `pnpm web:build` | `apps/web/dist` |
| Cloudflare Pages | `pnpm web:build` | `apps/web/dist` |

If GitHub Pages deploys under a repository path instead of a custom domain, set Astro's `base`
option in `astro.config.mjs` before building.
