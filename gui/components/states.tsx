export function LoadingState({ label = "リポジトリの証跡を読み込んでいます" }: { label?: string }) {
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
      <span className="state-code">API / 読み取り</span>
      <p>{message}</p>
    </div>
  );
}

export function EmptyState({ label = "記録なし", message }: { label?: string; message: string }) {
  return (
    <div className="state-card">
      <span className="state-code">{label}</span>
      <p>{message}</p>
    </div>
  );
}
