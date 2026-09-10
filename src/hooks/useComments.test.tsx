import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PrComment } from "@/types";
import { useComments } from "./useComments";

const { tauriCallMock } = vi.hoisted(() => ({
  tauriCallMock: vi.fn(),
}));

vi.mock("@/lib/tauri", () => ({
  tauriCall: tauriCallMock,
}));

function comment(id: string, content: string): PrComment {
  return {
    id,
    parentId: null,
    contentRaw: content,
    userDisplayName: "Reviewer",
    createdOn: "2026-09-01T00:00:00.000Z",
    deleted: false,
    inline: null,
  };
}

describe("useComments", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    tauriCallMock.mockReset();
  });

  it("keeps the selected PR's comments when an older request resolves later", async () => {
    let resolveA: (comments: PrComment[]) => void = () => {};
    let resolveB: (comments: PrComment[]) => void = () => {};

    tauriCallMock.mockImplementation((command: string, args?: { id?: number }) => {
      if (command !== "list_comments") {
        return Promise.reject(new Error("unsupported command"));
      }

      if (args?.id === 101) {
        return new Promise<PrComment[]>((resolve) => {
          resolveA = resolve;
        });
      }

      if (args?.id === 202) {
        return new Promise<PrComment[]>((resolve) => {
          resolveB = resolve;
        });
      }

      return Promise.reject(new Error(`unexpected PR id: ${String(args?.id)}`));
    });

    const { result, rerender } = renderHook(
      ({ prId }: { prId: number | null }) =>
        useComments("github", "example-workspace", "frontend-app", prId),
      { initialProps: { prId: 101 } },
    );

    await waitFor(() => expect(result.current.loading).toBe(true));
    rerender({ prId: 202 });
    await waitFor(() => expect(tauriCallMock).toHaveBeenCalledTimes(2));

    act(() => {
      resolveB([comment("9002", "For PR-202")]);
    });

    await waitFor(() => expect(result.current.comments).toEqual([comment("9002", "For PR-202")]));
    expect(result.current.loading).toBe(false);

    act(() => {
      resolveA([comment("9001", "For PR-101")]);
    });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.comments).toEqual([comment("9002", "For PR-202")]);
  });

  it("does not surface stale errors or loading from earlier requests", async () => {
    let rejectA: (error: Error) => void = () => {};
    let resolveB: (comments: PrComment[]) => void = () => {};

    tauriCallMock.mockImplementation((command: string, args?: { id?: number }) => {
      if (command !== "list_comments") {
        return Promise.reject(new Error("unsupported command"));
      }

      if (args?.id === 101) {
        return new Promise<PrComment[]>((_resolve, reject) => {
          rejectA = reject;
        });
      }

      if (args?.id === 202) {
        return new Promise<PrComment[]>((resolve) => {
          resolveB = resolve;
        });
      }

      return Promise.reject(new Error(`unexpected PR id: ${String(args?.id)}`));
    });

    const { result, rerender } = renderHook(
      ({ prId }: { prId: number | null }) =>
        useComments("github", "example-workspace", "frontend-app", prId),
      { initialProps: { prId: 101 } },
    );

    await waitFor(() => expect(result.current.loading).toBe(true));
    rerender({ prId: 202 });
    await waitFor(() => expect(tauriCallMock).toHaveBeenCalledTimes(2));

    act(() => {
      resolveB([comment("9002", "For PR-202")]);
    });

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBeNull();
    expect(result.current.comments).toEqual([comment("9002", "For PR-202")]);

    act(() => {
      rejectA(new Error("stale failure"));
    });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBeNull();
  });

  it("ignores explicit refresh calls from a stale selection", async () => {
    let resolveA: (comments: PrComment[]) => void = () => {};
    let resolveB: (comments: PrComment[]) => void = () => {};

    tauriCallMock.mockImplementation((command: string, args?: { id?: number }) => {
      if (command !== "list_comments") {
        return Promise.reject(new Error("unsupported command"));
      }

      if (args?.id === 101) {
        return new Promise<PrComment[]>((resolve) => {
          resolveA = resolve;
        });
      }

      if (args?.id === 202) {
        return new Promise<PrComment[]>((resolve) => {
          resolveB = resolve;
        });
      }

      return Promise.reject(new Error(`unexpected PR id: ${String(args?.id)}`));
    });

    const { result, rerender } = renderHook(
      ({ prId }: { prId: number | null }) =>
        useComments("github", "example-workspace", "frontend-app", prId),
      { initialProps: { prId: 101 } },
    );

    await waitFor(() => expect(result.current.loading).toBe(true));
    const staleRefresh = result.current.refresh;

    rerender({ prId: 202 });
    await waitFor(() => expect(tauriCallMock).toHaveBeenCalledTimes(2));

    act(() => {
      resolveB([comment("9002", "For PR-202")]);
    });
    await waitFor(() => expect(result.current.comments).toEqual([comment("9002", "For PR-202")]));

    const before = tauriCallMock.mock.calls.length;
    await act(async () => {
      await staleRefresh();
    });
    expect(tauriCallMock).toHaveBeenCalledTimes(before);
    expect(result.current.comments).toEqual([comment("9002", "For PR-202")]);
  });

  it("clears comments and errors when the selection is removed", async () => {
    tauriCallMock.mockResolvedValue([comment("9001", "For PR-101")]);
    const { result, rerender } = renderHook(
      ({ prId }: { prId: number | null }) =>
        useComments("github", "example-workspace", "frontend-app", prId),
      { initialProps: { prId: 101 } },
    );

    await waitFor(() => expect(result.current.comments).toHaveLength(1));

    act(() => {
      rerender({ prId: null });
    });

    expect(result.current.loading).toBe(false);
    expect(result.current.comments).toEqual([]);
    expect(result.current.error).toBeNull();
  });
});
