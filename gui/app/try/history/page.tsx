import { Shell } from "../../../components/shell";
import { TrialPageNavigation } from "../../../components/trial-page-nav";
import { TrialRun } from "../../../components/trial-run";

export default function TrialHistoryPage() {
  return (
    <Shell
      active="try"
      title="トライアル実行履歴"
      description="設定された実行ルートのセッションを要約し、進行状況または結果詳細へ移動します。"
    >
      <TrialPageNavigation active="history" />
      <TrialRun surface="history" />
    </Shell>
  );
}
