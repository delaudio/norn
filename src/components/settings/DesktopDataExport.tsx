import { useState } from "react";
import { Button } from "@/components/ui/button";
import { downloadDesktopData } from "@/lib/desktopDataExport";

export function DesktopDataExport() {
  const [status, setStatus] = useState<string | null>(null);
  return (
    <div className="border-b border-border px-4 py-3">
      <Button
        variant="outline"
        onClick={() => {
          try {
            const count = downloadDesktopData();
            setStatus(
              `Export requested for ${count} user-data entries. Verify the downloaded file before importing. Existing data remains unchanged.`,
            );
          } catch {
            setStatus(
              "Export failed. Preserve the webview profile and retry; no source data was removed.",
            );
          }
        }}
      >
        Export local drafts, references and preferences
      </Button>
      {status ? (
        <p role="status" className="mt-2 text-sm text-muted-foreground">
          {status}
        </p>
      ) : null}
    </div>
  );
}
