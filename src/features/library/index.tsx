import type { Feature } from "../registry";
import { useImportActions } from "../analyze/useImportActions";
import { LapLibraryPage } from "./LapLibraryPage";

function LibraryHeaderActions() {
  const { busy, handleImport, handleScan } = useImportActions();

  return (
    <>
      <button className="btn" onClick={handleScan} disabled={busy}>
        Scan folder
      </button>
      <button className="btn btn-primary" onClick={handleImport} disabled={busy}>
        Import IBT
      </button>
    </>
  );
}

export const libraryFeature: Feature = {
  id: "library",
  label: "Lap Library",
  path: "/library",
  element: <LapLibraryPage />,
  HeaderActions: LibraryHeaderActions,
};
