import { Shell } from "../../components/shell";
import { TrialPageNavigation } from "../../components/trial-page-nav";
import { TrialRun } from "../../components/trial-run";

export default function TrialRunPage() {
  return (
    <Shell
      active="try"
      title="トライアル実行指示"
      description="新しい実行内容を入力し、Gate 1 の固定内容を確認してから CLI を起動します。"
    >
      <TrialPageNavigation active="compose" />
      <TrialRun surface="compose" />
    </Shell>
  );
}
