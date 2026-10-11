import type { Question } from "../state/editing";
export function UnsavedDialog({
  question,
  answer,
}: {
  question?: Question;
  answer: (v: string) => void;
}) {
  if (!question) return null;
  return (
    <div className="modal-backdrop">
      <section
        role="dialog"
        aria-modal="true"
        aria-label={
          question.kind === "unsaved" ? "Save changes?" : "Export compatibility"
        }
        className="modal"
      >
        {question.kind === "unsaved" ? (
          <>
            <h2>Save changes?</h2>
            <p>Save this working game before continuing?</p>
            <button autoFocus onClick={() => answer("save")}>
              Save
            </button>
            <button onClick={() => answer("discard")}>Discard</button>
          </>
        ) : (
          <>
            <h2>Export compatibility</h2>
            <p>
              This PGN includes decoded moves and metadata, plus your edits.
              Original CBH annotations and unsupported content are omitted.
            </p>
            <button autoFocus onClick={() => answer("export")}>
              Export decoded content
            </button>
          </>
        )}
        <button onClick={() => answer("cancel")}>Cancel</button>
      </section>
    </div>
  );
}
