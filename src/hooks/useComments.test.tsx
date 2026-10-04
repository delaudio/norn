import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { tauriCall } from "@/lib/tauri";
import type { PrComment } from "@/types";
import { useComments } from "./useComments";

vi.mock("@/lib/tauri", () => ({ tauriCall: vi.fn() }));

const mockedTauriCall = vi.mocked(tauriCall);

function comment(id: string): PrComment {
  return {
    id,
    parentId: null,
    contentRaw: id,
    userDisplayName: "reviewer",
    createdOn: "2026-10-04T00:00:00.000Z",
    deleted: false,
    inline: null,
  };
}

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (error: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("useComments request lifecycle", () => {
  beforeEach(() => {
    mockedTauriCall.mockReset();
  });

  it("ignores an out-of-order response from a previously selected pull request", async () => {
    const first = deferred<PrComment[]>();
    const second = deferred<PrComment[]>();
    mockedTauriCall.mockImplementation((_command, args) => {
      const value = args as { id: number };
      return value.id === 1 ? first.promise : second.promise;
    });

    const { result, rerender } = renderHook(
      ({ prId }: { prId: number }) => useComments("github", "workspace", "repo", prId),
      { initialProps: { prId: 1 } },
    );

    await waitFor(() => expect(mockedTauriCall).toHaveBeenCalledTimes(1));

    rerender({ prId: 2 });
    await waitFor(() => expect(mockedTauriCall).toHaveBeenCalledTimes(2));

    await act(async () => {
      second.resolve([comment("from-pr-2")]);
    });
    await act(async () => {
      first.resolve([comment("from-pr-1")]);
    });

    expect(result.current.comments.map((entry) => entry.id)).toEqual(["from-pr-2"]);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("clears comments from the previous target before the next request resolves", async () => {
    const first = deferred<PrComment[]>();
    const second = deferred<PrComment[]>();
    mockedTauriCall.mockImplementation((_command, args) => {
      const value = args as { id: number };
      return value.id === 1 ? first.promise : second.promise;
    });

    const { result, rerender } = renderHook(
      ({ prId }: { prId: number }) => useComments("github", "workspace", "repo", prId),
      { initialProps: { prId: 1 } },
    );

    await act(async () => {
      first.resolve([comment("from-pr-1")]);
    });
    expect(result.current.comments.map((entry) => entry.id)).toEqual(["from-pr-1"]);

    rerender({ prId: 2 });
    expect(result.current.comments).toEqual([]);

    await act(async () => {
      second.resolve([comment("from-pr-2")]);
    });
    expect(result.current.comments.map((entry) => entry.id)).toEqual(["from-pr-2"]);
  });

  it("does not apply a stale error or loading update to the current selection", async () => {
    const first = deferred<PrComment[]>();
    const second = deferred<PrComment[]>();
    mockedTauriCall.mockImplementation((_command, args) => {
      const value = args as { id: number };
      return value.id === 1 ? first.promise : second.promise;
    });

    const { result, rerender } = renderHook(
      ({ prId }: { prId: number }) => useComments("github", "workspace", "repo", prId),
      { initialProps: { prId: 1 } },
    );

    await waitFor(() => expect(mockedTauriCall).toHaveBeenCalledTimes(1));
    rerender({ prId: 2 });
    await waitFor(() => expect(mockedTauriCall).toHaveBeenCalledTimes(2));

    await act(async () => {
      second.resolve([comment("from-pr-2")]);
    });
    await act(async () => {
      first.reject(new Error("stale failure"));
    });

    expect(result.current.comments.map((entry) => entry.id)).toEqual(["from-pr-2"]);
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it("clears comments and ignores in-flight responses when the target is deselected", async () => {
    const first = deferred<PrComment[]>();
    const second = deferred<PrComment[]>();
    mockedTauriCall.mockImplementation((_command, args) => {
      const value = args as { id: number };
      return value.id === 1 ? first.promise : second.promise;
    });

    const { result, rerender } = renderHook(
      ({ prId }: { prId: number | null }) => useComments("github", "workspace", "repo", prId),
      { initialProps: { prId: 1 as number | null } },
    );

    await act(async () => {
      first.resolve([comment("from-pr-1")]);
    });
    expect(result.current.comments).toHaveLength(1);

    rerender({ prId: 2 });
    await waitFor(() => expect(mockedTauriCall).toHaveBeenCalledTimes(2));
    rerender({ prId: null });

    expect(result.current.comments).toEqual([]);
    expect(result.current.loading).toBe(false);

    await act(async () => {
      second.resolve([comment("from-pr-2")]);
    });
    expect(result.current.comments).toEqual([]);
  });

  it("protects explicit refresh calls from an earlier in-flight request", async () => {
    const first = deferred<PrComment[]>();
    const second = deferred<PrComment[]>();
    let callCount = 0;
    mockedTauriCall.mockImplementation(() => {
      callCount += 1;
      return callCount === 1 ? first.promise : second.promise;
    });

    const { result } = renderHook(() => useComments("github", "workspace", "repo", 1));

    await waitFor(() => expect(mockedTauriCall).toHaveBeenCalledTimes(1));

    await act(async () => {
      void result.current.refresh();
    });
    await waitFor(() => expect(mockedTauriCall).toHaveBeenCalledTimes(2));

    await act(async () => {
      second.resolve([comment("fresh")]);
    });
    await act(async () => {
      first.resolve([comment("initial")]);
    });

    expect(result.current.comments.map((entry) => entry.id)).toEqual(["fresh"]);
  });

  it("invalidates requests when the repository or provider changes", async () => {
    const repository = deferred<PrComment[]>();
    const provider = deferred<PrComment[]>();
    mockedTauriCall.mockImplementation((_command, args) => {
      const value = args as { provider: string };
      return value.provider === "github" ? repository.promise : provider.promise;
    });

    const { result, rerender } = renderHook(
      ({ repo, selected }: { repo: string; selected: "github" | "bitbucket" }) =>
        useComments(selected, "workspace", repo, 1),
      { initialProps: { repo: "repo", selected: "github" as "github" | "bitbucket" } },
    );

    await waitFor(() => expect(mockedTauriCall).toHaveBeenCalledTimes(1));

    rerender({ repo: "repo", selected: "bitbucket" });
    await waitFor(() => expect(mockedTauriCall).toHaveBeenCalledTimes(2));

    await act(async () => {
      provider.resolve([comment("bitbucket-pr")]);
    });
    await act(async () => {
      repository.resolve([comment("github-pr")]);
    });

    expect(result.current.comments.map((entry) => entry.id)).toEqual(["bitbucket-pr"]);
  });
});
