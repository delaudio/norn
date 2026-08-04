import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://docs.lachesi.dev",
  integrations: [
    starlight({
      title: "Norn Docs",
      description: "Local-first pull request review workspace for Bitbucket Cloud and GitHub.",
      logo: {
        src: "./src/assets/norn.svg",
        alt: "Norn",
      },
      customCss: ["./src/styles/starlight.css"],
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/lachesi-hq/lachesi",
        },
      ],
      editLink: {
        baseUrl: "https://github.com/lachesi-hq/lachesi/edit/main/apps/docs/",
      },
      sidebar: [
        {
          label: "Start Here",
          items: [
            { label: "Overview", slug: "overview" },
            { label: "Getting Started", slug: "getting-started" },
          ],
        },
        {
          label: "Using Norn",
          items: [
            { label: "Providers", slug: "providers" },
            { label: "Configuration", slug: "configuration" },
            { label: "Review Workflow", slug: "review-workflow" },
          ],
        },
        {
          label: "Reference",
          items: [
            { label: "Storybook", slug: "storybook" },
            { label: "Roadmap", slug: "roadmap" },
          ],
        },
      ],
    }),
  ],
});
