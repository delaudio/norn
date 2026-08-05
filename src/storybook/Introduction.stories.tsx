import type { Meta, StoryObj } from "@storybook/react-vite";

function Introduction() {
  return (
    <main className="min-h-screen bg-background text-foreground">
      <section className="border-b border-border bg-secondary px-8 py-14">
        <div className="mx-auto max-w-5xl">
          <p className="mb-3 text-sm font-medium text-muted-foreground">Norn Design System</p>
          <h1 className="max-w-3xl text-4xl font-semibold tracking-normal">
            Product UI patterns for local-first pull request review.
          </h1>
          <p className="mt-5 max-w-2xl text-base leading-7 text-muted-foreground">
            This Storybook documents Norn components, app surfaces, review states, and interaction
            patterns used by the desktop review workspace.
          </p>
        </div>
      </section>

      <section className="mx-auto grid max-w-5xl gap-8 px-8 py-10 md:grid-cols-[1.2fr_0.8fr]">
        <div>
          <h2 className="text-lg font-semibold">What is covered</h2>
          <div className="mt-5 grid gap-3">
            {[
              "Application shell, navigation, and review workspace layout",
              "Pull request sidebar, filtering states, and repository grouping",
              "Diff review surfaces, file tree behavior, and image/text preview states",
              "Comment composition, pending review actions, and discussion threads",
              "Settings flows for providers, repositories, credentials, and preferences",
            ].map((item) => (
              <div
                key={item}
                className="rounded-md border border-border bg-card px-4 py-3 text-sm text-card-foreground"
              >
                {item}
              </div>
            ))}
          </div>
        </div>

        <aside className="rounded-md border border-border bg-card p-5">
          <h2 className="text-lg font-semibold">Publication target</h2>
          <dl className="mt-5 space-y-4 text-sm">
            <div>
              <dt className="text-muted-foreground">URL</dt>
              <dd className="mt-1 font-mono text-foreground">design-system.norn.dev</dd>
            </div>
            <div>
              <dt className="text-muted-foreground">Build output</dt>
              <dd className="mt-1 font-mono text-foreground">storybook-static/</dd>
            </div>
            <div>
              <dt className="text-muted-foreground">Theme</dt>
              <dd className="mt-1 text-foreground">Light and dark via the Storybook toolbar</dd>
            </div>
          </dl>
        </aside>
      </section>
    </main>
  );
}

const meta = {
  title: "Design System/Introduction",
  component: Introduction,
  parameters: {
    layout: "fullscreen",
  },
} satisfies Meta<typeof Introduction>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Overview: Story = {};
