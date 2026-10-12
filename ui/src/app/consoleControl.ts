// UI-only Win32 signal control. The Rust engine still communicates over stdio;
// no engine library or Rust memory is loaded into the UI process.
export interface ConsoleControl {
  protect(): void;
  close(): void;
}

export function createConsoleControl(
  setIgnored: (ignored: boolean) => void,
  release: () => void,
): ConsoleControl {
  let closed = false;
  return {
    protect() {
      if (closed) throw new Error("The native console control is closed.");
      setIgnored(true);
    },
    close() {
      if (closed) return;
      closed = true;
      try {
        setIgnored(false);
      } finally {
        release();
      }
    },
  };
}

let control: ConsoleControl | null = null;
export async function protectConsoleInterrupts(): Promise<void> {
  if (process.platform !== "win32") return;
  if (!control) {
    const { dlopen, FFIType } = await import("bun:ffi");
    const library = dlopen("kernel32.dll", {
      SetConsoleCtrlHandler: { args: [FFIType.ptr, FFIType.i32], returns: FFIType.i32 },
    });
    control = createConsoleControl(
      (ignored) => {
        if (library.symbols.SetConsoleCtrlHandler(null, ignored ? 1 : 0) === 0)
          throw new Error("The native console interrupt control is unavailable.");
      },
      () => library.close(),
    );
  }
  // Reassert after renderer suspension, which changes native input handling.
  control.protect();
}

export function restoreConsoleInterrupts(): void {
  const previous = control;
  control = null;
  previous?.close();
}
