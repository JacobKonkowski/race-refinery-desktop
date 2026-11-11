import { useState } from "react";
import type { LapSetSummary } from "../../shared/types";
import { formatDate } from "../../shared/format";

interface Props {
  sets: LapSetSummary[];
  activeSetId: number | null;
  onOpen: (setId: number) => void;
  onRename: (setId: number, name: string) => void;
  onDelete: (setId: number) => void;
}

export function SetsPanel({ sets, activeSetId, onOpen, onRename, onDelete }: Props) {
  const [renamingId, setRenamingId] = useState<number | null>(null);
  const [renameValue, setRenameValue] = useState("");

  function startRename(set: LapSetSummary) {
    setRenamingId(set.id);
    setRenameValue(set.name);
  }

  function commitRename(setId: number) {
    const name = renameValue.trim();
    if (!name) return;
    onRename(setId, name);
    setRenamingId(null);
  }

  return (
    <section className="library-sets">
      <div className="panel-header">
        <h2>Sets</h2>
      </div>
      {sets.length === 0 ? (
        <p className="muted library-sets-empty">No saved sets yet. Check laps and Save as set.</p>
      ) : (
        <ul className="library-sets-list">
          {sets.map((set) => {
            const renaming = renamingId === set.id;
            return (
              <li
                key={set.id}
                className={`library-set-row${activeSetId === set.id ? " active" : ""}`}
              >
                <div className="library-set-main">
                  {renaming ? (
                    <input
                      className="selection-title-input"
                      value={renameValue}
                      autoFocus
                      aria-label="Set name"
                      onChange={(e) => setRenameValue(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") commitRename(set.id);
                        if (e.key === "Escape") setRenamingId(null);
                      }}
                    />
                  ) : (
                    <button
                      type="button"
                      className="library-set-name"
                      onClick={() => onOpen(set.id)}
                      title="Load into selection"
                    >
                      <span>{set.name}</span>
                      <span className="muted">
                        {set.track} · {set.lapCount} lap{set.lapCount === 1 ? "" : "s"} ·{" "}
                        {formatDate(set.createdAt)}
                      </span>
                    </button>
                  )}
                </div>
                <div className="library-set-actions">
                  {renaming ? (
                    <>
                      <button type="button" className="btn" onClick={() => commitRename(set.id)}>
                        OK
                      </button>
                      <button
                        type="button"
                        className="btn btn-ghost"
                        onClick={() => setRenamingId(null)}
                      >
                        Cancel
                      </button>
                    </>
                  ) : (
                    <>
                      <button type="button" className="btn" onClick={() => onOpen(set.id)}>
                        Open
                      </button>
                      <button type="button" className="btn btn-ghost" onClick={() => startRename(set)}>
                        Rename
                      </button>
                      <button
                        type="button"
                        className="btn btn-ghost btn-danger"
                        onClick={() => onDelete(set.id)}
                      >
                        Delete
                      </button>
                    </>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
