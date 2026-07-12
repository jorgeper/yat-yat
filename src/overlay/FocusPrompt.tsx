// The focus-guard prompt (SPEC7 FR-G3): focus moved to a different app while
// dictating, so nothing pastes until the user decides. Rendered inside the
// non-activating overlay pill — clicks never steal focus from the target app.

export default function FocusPrompt({
  fromApp,
  toApp,
  onPaste,
  onCopy,
}: {
  fromApp: string;
  toApp: string;
  onPaste: () => void;
  onCopy: () => void;
}) {
  return (
    <div className="focus-prompt" data-testid="focus-prompt">
      <span className="pill-label focus-question" data-testid="focus-question">
        Started in <strong data-testid="focus-from">{fromApp}</strong> — paste into{" "}
        <strong data-testid="focus-to">{toApp}</strong>?
      </span>
      <div className="focus-actions">
        <button className="btn primary" data-testid="focus-paste-btn" onClick={onPaste}>
          Paste
        </button>
        <button className="btn" data-testid="focus-copy-btn" onClick={onCopy}>
          Copy only
        </button>
      </div>
    </div>
  );
}
