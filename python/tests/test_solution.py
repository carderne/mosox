import math
from pathlib import Path

import pandas as pd
import pytest

import mosox

TRANSPORT = mosox.Model.from_file(Path(__file__).parents[2] / "examples/2d_params/model.mod")


@pytest.mark.parametrize("file_format", ["parquet", "csv"])
def test_write_read(tmp_path: Path, file_format: mosox.Format) -> None:
    solution = TRANSPORT.solve()
    solution.write(tmp_path, file_format=file_format)
    assert (tmp_path / f"ship.{file_format}").exists()
    read = mosox.Solution.read(tmp_path)
    assert (read.status, read.objective, read.stats) == (solution.status, solution.objective, solution.stats)
    assert list(read.variables) == ["ship"]
    assert list(read.constraints) == ["supply_limit", "meet_demand"]
    # CSV doesn't keep types
    pd.testing.assert_frame_equal(read["ship"], solution["ship"], check_dtype=file_format == "parquet")
    assert read.arrow("ship").num_rows == 6


def test_write_read_infeasible(tmp_path: Path) -> None:
    TRANSPORT.solve({"supply": 1}).write(tmp_path)
    read = mosox.Solution.read(tmp_path)
    assert read.status == "infeasible" and math.isnan(read.objective)
    with pytest.raises(KeyError):
        read["ship"]
