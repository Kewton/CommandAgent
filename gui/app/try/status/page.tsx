import { Shell } from "../../../components/shell";
import { TrialPageNavigation } from "../../../components/trial-page-nav";
import { TrialRun } from "../../../components/trial-run";

export default function TrialStatusPage() {
  return (
    <Shell
      active="try"
      title="トライアル実行状況"
      description="対象セッションへ読み取り専用で再接続し、Gate 2 の進行状況とイベントを確認します。"
    >
      <TrialPageNavigation active="status" />
      <TrialRun surface="status" />
    </Shell>
  );
}
