import { Shell } from "../../../../components/shell";
import { TrialPageNavigation } from "../../../../components/trial-page-nav";
import { TrialRun } from "../../../../components/trial-run";

export default function TrialDetailPage() {
  return (
    <Shell
      active="try"
      title="トライアル実行結果詳細"
      description="terminal verdict、失敗診断、受入シート、イベント、成果物を対象セッションごとに確認します。"
    >
      <TrialPageNavigation active="detail" />
      <TrialRun surface="detail" />
    </Shell>
  );
}
