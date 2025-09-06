# Solver-Flowlight Agent Guidelines

## 言語ポリシー（重要）
- 回答・説明・提案は常に日本語で行うこと。
- コミットメッセージやPR説明、コメントも原則日本語を用いること。
- 例外: 外部API仕様、コード識別子、ログの固定英語出力はそのままで可。
  その場合も補足説明は日本語で記載すること。

## Auto-Formatting Rule

- After modifying `solver-flowlight/src/main.rs`, always run Rust formatter.
- Command:
  - `cargo fmt -p icfpc_solver_flowlight -- solver-flowlight/src/main.rs`
- Purpose: keep diffs tidy and consistent across contributors and tools.

## One-Time Setup

- Ensure rustfmt component is available for the workspace toolchain (Rust 1.89.0):
  - `rustup component add rustfmt --toolchain 1.89.0`
- If using a pinned toolchain via `rust-toolchain.toml`, no extra flags are required once rustfmt is installed.

## Edit Flow (for agents and humans)

- Make code changes (e.g., via `apply_patch`).
- Run formatter on the touched file:
  - `cargo fmt -p icfpc_solver_flowlight -- solver-flowlight/src/main.rs`
- Optionally verify formatting in CI/local checks:
  - `cargo fmt --check -p icfpc_solver_flowlight`

## Notes

- Only format the Flowlight crate unless explicitly asked to format the whole workspace.
- If rustfmt is missing, install it as above before attempting to format.
