---
title: Storybook
description: Design system documentation and static deployment.
---

Norn has a Storybook build for design system and component documentation.

## Local Development

```sh
pnpm storybook
```

## Static Build

```sh
pnpm storybook:build
```

The static output is written to `storybook-static/`.

## Deployment

The public Storybook target is:

```txt
https://design-system.norn.dev
```

Deployment is configured through `wrangler.design-system.jsonc` and can be run with:

```sh
pnpm storybook:deploy
```
