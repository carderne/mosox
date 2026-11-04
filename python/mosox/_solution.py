import json
import math
from collections.abc import Iterator, Mapping
from dataclasses import asdict, dataclass
from os import PathLike
from pathlib import Path
from typing import Any, Literal, Self

import pandas as pd
import pyarrow as pa
import pyarrow.csv
import pyarrow.parquet

Format = Literal["parquet", "csv"]

_META = "meta.json"


@dataclass(frozen=True)
class Stats:
    num_rows: int
    """Including the objective"""
    num_cols: int
    num_nonzeros: int
    compile_seconds: float
    solve_seconds: float


class Solution:
    """A solved model: its status, objective, and a table for each variable and constraint.

    Tables have the index columns, then `VALUE` and `MARGINAL`. Index columns are named for
    their sets (`REGION` in `{r in REGION}`), or dummies where a set is repeated.
    If no feasible solution was found, `objective` is NaN and there are no tables."""

    status: str
    """HiGHS model status, eg `optimal`, `infeasible`, `time_limit`"""
    objective: float
    stats: Stats
    variables: Mapping[str, pd.DataFrame]
    constraints: Mapping[str, pd.DataFrame]

    def __init__(
        self,
        status: str,
        objective: float,
        stats: Stats,
        variables: Mapping[str, pa.Table | Path],
        constraints: Mapping[str, pa.Table | Path],
    ) -> None:
        self.status = status
        self.objective = objective
        self.stats = stats
        self.variables = _Frames(variables)
        self.constraints = _Frames(constraints)

    @property
    def optimal(self) -> bool:
        return self.status == "optimal"

    def __getitem__(self, name: str) -> pd.DataFrame:
        """A variable or constraint table"""
        return self._frames(name)[name]

    def arrow(self, name: str) -> pa.Table:
        """A variable or constraint table, without converting to pandas"""
        return self._frames(name).arrow(name)

    def write(self, path: str | PathLike[str], *, file_format: Format = "parquet") -> None:
        """Write to a directory: a file per table (as otoole lays out results), and `meta.json`"""
        _write(Path(path), self, file_format, {})

    @classmethod
    def read(cls, path: str | PathLike[str]) -> Self:
        """Read from a directory written by `write`. Tables are read when first used."""
        path = Path(path)
        meta = json.loads((path / _META).read_text())
        ext = meta["format"]
        return cls(
            status=meta["status"],
            objective=math.nan if meta["objective"] is None else meta["objective"],
            stats=Stats(**meta["stats"]),
            variables={name: path / f"{name}.{ext}" for name in meta["variables"]},
            constraints={name: path / f"{name}.{ext}" for name in meta["constraints"]},
        )

    def __repr__(self) -> str:
        return f"Solution(status={self.status!r}, objective={self.objective})"

    def _frames(self, name: str) -> "_Frames":
        for frames in (self.variables, self.constraints):
            if name in frames:
                assert isinstance(frames, _Frames)
                return frames
        raise KeyError(name)


class _Frames(Mapping[str, pd.DataFrame]):
    """Tables by name, read and converted to pandas when first used"""

    def __init__(self, tables: Mapping[str, pa.Table | Path]) -> None:
        self._tables = dict(tables)
        self._frames: dict[str, pd.DataFrame] = {}

    def arrow(self, name: str) -> pa.Table:
        table = self._tables[name]
        if isinstance(table, Path):
            table = self._tables[name] = _read_table(table)
        return table

    def __getitem__(self, name: str) -> pd.DataFrame:
        if name not in self._frames:
            self._frames[name] = self.arrow(name).to_pandas()
        return self._frames[name]

    def __iter__(self) -> Iterator[str]:
        return iter(self._tables)

    def __len__(self) -> int:
        return len(self._tables)


def _write(path: Path, solution: Solution | None, file_format: Format, extra: Mapping[str, Any]) -> None:
    """Write a solution (if any) with extra metadata, writing `meta.json` last so it marks completion"""
    path.mkdir(parents=True, exist_ok=True)
    (path / _META).unlink(missing_ok=True)
    meta: dict[str, Any] = {"format": file_format, "variables": [], "constraints": [], **extra}
    if solution is not None:
        for name in [*solution.variables, *solution.constraints]:
            table = solution.arrow(name)
            if file_format == "parquet":
                pyarrow.parquet.write_table(table, path / f"{name}.parquet")
            else:
                pyarrow.csv.write_csv(table, path / f"{name}.csv")
        meta |= {
            "status": solution.status,
            "objective": None if math.isnan(solution.objective) else solution.objective,
            "stats": asdict(solution.stats),
            "variables": list(solution.variables),
            "constraints": list(solution.constraints),
        }
    (path / _META).write_text(json.dumps(meta, indent=2))


def _read_meta(path: Path) -> dict[str, Any] | None:
    meta = path / _META
    return json.loads(meta.read_text()) if meta.exists() else None


def _read_table(path: Path) -> pa.Table:
    if path.suffix == ".parquet":
        return pyarrow.parquet.read_table(path)
    return pyarrow.csv.read_csv(path)
