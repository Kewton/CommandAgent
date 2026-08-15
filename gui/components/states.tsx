export function LoadingState({ label = "Loading repository evidence" }: { label?: string }) {
  return (
    <div className="state-card" role="status">
      <span className="loader" />
      <p>{label}</p>
    </div>
  );
}

export function ErrorState({ message }: { message: string }) {
  return (
    <div className="state-card error-card" role="alert">
      <span className="state-code">API / READ</span>
      <p>{message}</p>
    </div>
  );
}

export function EmptyState({ message }: { message: string }) {
  return (
    <div className="state-card">
      <span className="state-code">NO RECORDS</span>
      <p>{message}</p>
    </div>
  );
}
