# Profile resolution

profile は repository 名の別名ではなく、git metadata から deletion proof の基準を解決する規則である。path や branch 名を固定値として持たない。

## commandagent

1. current repository は `git rev-parse --show-toplevel` と `git worktree list --porcelain` から解決する。
2. remote は対象 base branch の upstream が属する remote を使う。upstream が無い場合だけ `git remote -v` と利用者指定から解決し、曖昧なら停止する。
3. base branch は integration worktree の checked-out branch と upstream から解決する。この repository で観測される `develop` を一般規則として hardcode しない。
4. integration worktree は base branch を checked out している worktree、main worktree は porcelain 一覧の最初の primary entry と repository metadata から識別する。current / main / integration は重複しても各保護条件を満たしたものとして除外する。
5. baseline は base と異なる tree 比較基準が repository 設定または利用者入力にある場合だけ設定し、それ以外は null にする。

## commandmate

同じ git metadata 規則を使う。CommandMate の worktree id や path naming は selection の補助情報に留め、branch/base/integration 判定の truth にしない。

## 利用者指定 profile

resolved remote、base、integration worktree、baseline と、それぞれの discovery source を dry-run summary に示す。未知 repository では `verified: false` とし、利用者がその解決値を確認するまで apply しない。
