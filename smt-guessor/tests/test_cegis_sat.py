import json
from pathlib import Path
import sys

import pytest

# Ensure repository root is on sys.path for direct module import
ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import cegis_sat as cs
import shutil


def test_cegis_sat_solves_probatio():
    data_path = Path("example/probatio.json")
    assert data_path.exists(), "example/probatio.json must exist"
    prob = cs.load_problem(str(data_path))
    plans = prob["plans"]
    results = prob["results"]
    N = int(prob["N"])  # type: ignore[index]

    # Choose backend: prefer kissat if available
    backend = "kissat" if cs.have_kissat_binary() else "pysat"
    if not cs.have_python_sat():
        pytest.skip("python-sat not available; cannot build CNF for SAT backends")
    out, meta = cs.cegis_sat(plans, results, N, init_prefix=8, max_iters=40, verbose=False, backend=backend)
    assert out is not None, f"CEGIS failed: {meta}"
    ok, errs = cs.verify_solution(plans, results, N, out)
    assert ok, f"solution does not verify: {errs}"
