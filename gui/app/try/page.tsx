import { Shell } from "../../components/shell";
import { TrialRun } from "../../components/trial-run";

export default function TrialRunPage() {
  return (
    <Shell
      active="try"
      title="トライアル"
      description="設定された execution root で GUI Trial を開始・監視し、.anvil/runs の履歴を確認します。"
    >
      <TrialRun />
    </Shell>
  );
}
