export function Toolbar({
  navigate,
  flip,
  enabled,
}: {
  navigate: (action: "first" | "previous" | "next" | "last") => void;
  flip: () => void;
  enabled: boolean;
}) {
  return (
    <div className="toolbar" aria-label="Move navigation">
      {(["first", "previous", "next", "last"] as const).map((action, i) => (
        <button
          key={action}
          aria-label={`${action[0].toUpperCase() + action.slice(1)} move`}
          disabled={!enabled}
          onClick={() => navigate(action)}
        >
          {["⏮", "←", "→", "⏭"][i]}
        </button>
      ))}
      <button onClick={flip}>Flip board</button>
    </div>
  );
}
