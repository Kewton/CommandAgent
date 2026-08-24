# v5 harvest

実機側 `d3a2_smoke5_origin2` を確認した。workflow-eventsの最終行は `workflow_node_run_created`（investigate）で、`workflow_adjudicated`はない。`evidence/investigate-events.jsonl`には`intent_resolved`のみがあり、指定UUID runディレクトリの`events.jsonl`、`run_stop`、investigate verdictは未生成。従ってv5は`circle_interrupted`（environment/session）である。

closure内の既存executor Configは作業ディレクトリ（リポジトリ）をworkspace rootとして使用し、`--origin`コピーをchild Configへworkspace rootとして伝播していない。証跡のみタスクではRust配線を変更できないため、v6の実run完走・run_id回収は未達として記録する。
