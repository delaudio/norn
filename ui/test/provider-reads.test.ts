import { expect, test } from "bun:test";
import type { ProviderPrListParams } from "../../src/protocol/generated/protocol";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const repository = { provider: "github", workspace: "Owner", repo: "Repo" } as const;
const params: ProviderPrListParams = { repository, filter: "DRAFT", page: 1, limit: 30 };

class ReadTransport extends FakeBackendTransport {
  calls: string[] = [];
  constructor(
    private readonly response: (
      method: string,
      params: Record<string, unknown>,
    ) => Record<string, unknown> | Promise<Record<string, unknown>>,
  ) {
    super();
  }
  async request(method: string, params: Record<string, unknown>) {
    this.calls.push(method);
    return this.response(method, params);
  }
}
test("typed provider reads normalize optional filters and preserve empty-page continuation", async () => {
  const transport = new ReadTransport((_method, request) => ({ request, values: [], nextPage: 2 }));
  const page = await new BackendClient(transport).providerPrs(params);
  expect(page.request).toEqual({ ...params, query: null, authorId: null });
  expect(page.values).toEqual([]);
  expect(page.nextPage).toBe(2);
  expect(transport.calls).toEqual(["provider.prs"]);
});
test("provider reads reject wrong provider/filter/cursor, unknown fields and oversized replies", async () => {
  for (const fault of ["provider", "filter", "cursor", "unknown", "oversize"] as const) {
    const transport = new ReadTransport((_method, request) => {
      const result: Record<string, unknown> = { request, values: [], nextPage: 2 };
      if (fault === "provider")
        result.request = { ...request, repository: { ...repository, provider: "bitbucket" } };
      if (fault === "filter") result.request = { ...request, filter: "ALL" };
      if (fault === "cursor") result.nextPage = 4;
      if (fault === "unknown") result.privateBody = "untrusted content";
      if (fault === "oversize") result.privateBody = "x".repeat(512 * 1024);
      return result;
    });
    await expect(new BackendClient(transport).providerPrs(params)).rejects.toThrow();
    expect(transport.calls).toHaveLength(1);
  }
});
test("generation fences late provider reads and invalid input never sends a request", async () => {
  let current = true;
  const transport = new ReadTransport((_method, request) => {
    current = false;
    return { request, values: [], nextPage: null };
  });
  await expect(new BackendClient(transport).providerPrs(params, () => current)).rejects.toThrow(
    "selection changed",
  );
  current = true;
  await expect(new BackendClient(transport).providerPrs({ ...params, page: 101 })).rejects.toThrow(
    "parameters",
  );
  expect(transport.calls).toHaveLength(1);
});
test("current-user echoes explicit repository/provider identity", async () => {
  const transport = new ReadTransport((_method, request) => ({
    repository: request.repository,
    accountId: "actual-author",
    displayName: "Actual Author",
  }));
  expect((await new BackendClient(transport).providerUser(repository)).accountId).toBe(
    "actual-author",
  );
  expect(transport.calls).toEqual(["provider.user"]);
});
test("detail verifies actual PR number and exact trusted browser URL", async () => {
  for (const fault of ["none", "id", "link"] as const) {
    const transport = new ReadTransport((_method, request) => ({
      request,
      authorId: "author",
      browserUrl:
        fault === "link"
          ? "https://github.com/Other/Repo/pull/7"
          : "https://github.com/Owner/Repo/pull/7",
      detail: {
        id: fault === "id" ? 8 : 7,
        title: "Actual PR",
        descriptionRaw: "Actual description",
        state: "OPEN",
        draft: false,
        authorDisplayName: "Author",
        sourceBranch: "feature",
        destinationBranch: "main",
        sourceCommitHash: "a".repeat(40),
        destinationCommitHash: "b".repeat(40),
        createdOn: "",
        updatedOn: "",
        reviewers: [],
      },
    }));
    const result = new BackendClient(transport).providerPrDetail({ repository, prId: 7 });
    if (fault === "none") expect((await result).detail.title).toBe("Actual PR");
    else await expect(result).rejects.toThrow();
  }
});

test("provider browser responses prove full request, resolved identity and canonical HTTPS URL", async () => {
  const request = { repository, prId: 7 };
  for (const fault of [
    "none",
    "scope",
    "repository",
    "host",
    "credentials",
    "port",
    "local",
    "launch",
  ]) {
    const transport = new ReadTransport((_method, params) => {
      const result: Record<string, unknown> = {
        request: params,
        repository,
        url: "https://github.com/Owner/Repo/pull/7",
        launchRequested: false,
        launchError: null,
      };
      if (fault === "scope") result.request = { ...params, prId: 8 };
      if (fault === "repository") result.repository = { ...repository, repo: "Other" };
      if (fault === "host") result.url = "https://evil.test/Owner/Repo/pull/7";
      if (fault === "credentials") result.url = "https://secret@github.com/Owner/Repo/pull/7";
      if (fault === "port") result.url = "https://github.com:1234/Owner/Repo/pull/7";
      if (fault === "local") result.url = "http://127.0.0.1:1234/session/private";
      if (fault === "launch") result.launchError = "failure";
      return result;
    });
    const result = new BackendClient(transport).providerBrowser(request);
    if (fault === "none") expect((await result).launchRequested).toBe(false);
    else await expect(result).rejects.toThrow();
  }
  const uuidRequest = {
    repository: {
      provider: "bitbucket",
      workspace: "{11111111-1111-1111-1111-111111111111}",
      repo: "{22222222-2222-2222-2222-222222222222}",
    } as const,
    prId: 7,
  };
  const transport = new ReadTransport((_method, params) => ({
    request: params,
    repository: { provider: "bitbucket", workspace: "Alias", repo: "Repo" },
    url: "https://bitbucket.org/Alias/Repo/pull-requests/7",
    launchRequested: true,
    launchError: "Owned opener unavailable",
  }));
  expect((await new BackendClient(transport).providerBrowser(uuidRequest)).launchError).toBe(
    "Owned opener unavailable",
  );
  await expect(
    new BackendClient(transport).providerBrowser({
      repository: { ...repository, workspace: ".." },
      prId: 7,
    }),
  ).rejects.toThrow();
  expect(transport.calls).toHaveLength(1);
});
