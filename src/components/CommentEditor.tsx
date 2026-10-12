import { useEffect, useState } from "react";
import type { GameNode, Edit } from "../api/types";
export function CommentEditor({
  node,
  disabled,
  edit,
  autoFocus,
}: {
  node: GameNode;
  disabled: boolean;
  edit: (e: Edit) => Promise<unknown>;
  autoFocus?: boolean;
}) {
  const [comments, setComments] = useState(""),
    [nags, setNags] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    setComments(node.comments.join("\n"));
    setNags(node.nags.join(" "));
    setError("");
  }, [node]);
  return (
    <div className="comment-editor">
      <label>
        Comments
        <textarea
          value={comments}
          disabled={disabled}
          autoFocus={autoFocus}
          onChange={(e) => setComments(e.target.value)}
        />
      </label>
      <button
        disabled={disabled}
        onClick={() =>
          void edit({
            kind: "setComments",
            node: node.id,
            comments: comments ? comments.split("\n") : [],
          })
        }
      >
        Apply comments
      </button>
      <label>
        NAGs
        <input
          value={nags}
          disabled={disabled}
          onChange={(e) => setNags(e.target.value)}
        />
      </label>
      <button
        disabled={disabled}
        onClick={() => {
          const tokens = nags.trim() ? nags.trim().split(/\s+/) : [];
          if (tokens.some((v) => !/^\d+$/.test(v) || Number(v) > 255)) {
            setError("NAGs must be integers from 0 to 255.");
            return;
          }
          setError("");
          void edit({
            kind: "setNags",
            node: node.id,
            nags: tokens.map(Number),
          });
        }}
      >
        Apply NAGs
      </button>
      {error && <p role="alert">{error}</p>}
    </div>
  );
}
