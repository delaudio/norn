# Storybook Publishing On Cloudflare

Norn Storybook is prepared for a public static deployment on Cloudflare Pages at:

```txt
https://design-system.lachesi.dev
```

## Build

```sh
pnpm storybook:build
```

The static output is written to `storybook-static/`.

## Cloudflare Pages

Create a Cloudflare Pages project connected to the GitHub repository.

Use these build settings:

```txt
Framework preset: None
Build command: pnpm storybook:build
Build output directory: storybook-static
Root directory: /
Production branch: main
```

Cloudflare Pages starts from the repository root unless a root directory is configured, then uploads the build output directory as the site contents.

## Cloudflare Workers Static Assets

If the Cloudflare project requires a deploy command, use the dedicated Wrangler config instead of bare `wrangler deploy`.

Use these build settings:

```txt
Build command: pnpm storybook:build
Deploy command: npx wrangler deploy --config wrangler.design-system.jsonc
```

The config in `wrangler.design-system.jsonc` points Wrangler at `storybook-static/` explicitly, so Wrangler does not try to infer the monorepo's Astro app settings.

The Cloudflare API token for this flow must be able to deploy Workers. A Pages-only token is not enough.

## Custom Domain

Attach this custom domain to the Cloudflare Pages project:

```txt
design-system.lachesi.dev
```

If `lachesi.dev` is already managed in Cloudflare DNS, Cloudflare can create or validate the required DNS record from the Pages custom domain flow.

## Static Headers

`public/storybook/_headers` is copied into the Storybook static output. Cloudflare Pages parses that file and applies the configured static response headers.
