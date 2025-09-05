# Repository Guidelines

## Project Structure & Module Organization
- `main.py`: Core SMT solver using Z3; reads trace JSON, reconstructs rooms and connections, writes map JSON.
- `visualize.py`: Hex‑grid renderer for a produced map; saves PNGs.
- `example/`: Small inputs and outputs (`sample.json`, `map.json`).
- `pyproject.toml` / `uv.lock`: Python dependencies (Z3, Matplotlib). Python ≥ 3.9.

## Setup, Build, Test, and Development Commands
- Environment: create and activate a venv, then install deps.
  - With `uv` (preferred if installed): `uv sync`
  - With `pip`: `pip install z3-solver matplotlib`
- Run solver: `python main.py --json example/sample.json --output out.map.json`
- Run visualizer: `python visualize.py --input example/map.json --output map.png`
- Ad‑hoc check: open `out.map.json` and verify `rooms` length and `connections` symmetry.

## Coding Style & Naming Conventions
- Python style: 4‑space indents, PEP 8 naming (functions `snake_case`, classes `CapWords`, constants `UPPER_CASE`).
- Type hints and short docstrings for public functions.
- Imports: standard lib, third‑party, local — in that order.
- Optional tooling (not enforced): `black .` and `ruff check .` before pushing.

## Testing Guidelines
- Framework: use `pytest` if adding tests.
- Layout: put tests under `tests/` with files named `test_*.py`.
- Quick example: `pytest -q` runs tests; add one golden test that feeds `example/sample.json` to `main.py` and asserts key fields exist in the output map.

## Commit & Pull Request Guidelines
- Commits: imperative mood and focused (e.g., "Add curved edge renderer", "Fix plan normalization for 1–6").
- Branches: short, descriptive (e.g., `feat/visual-curves`, `fix/normalize-plan`).
- PRs: include a concise description, reproduction commands, and before/after artifacts (e.g., rendered PNG). Link related issues.

## Architecture Overview & Tips
- Model: ports `(room, door)` with an involutive `delta` and 2‑bit `label(room)`; traces observe labels after entering next room.
- Performance: large `--maxN` sweeps can be slow; prefer supplying `--N` when known.
- Reproducibility: keep new minimal examples in `example/` and avoid committing large binary images.
