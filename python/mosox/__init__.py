"""Python bindings for mosox, an LP matrix generator for GMPL."""

from collections.abc import Mapping
from dataclasses import dataclass
from os import PathLike
from typing import Self

import pandas as pd
import pyarrow as pa

# The Rust bindings are in `_mosox`; all access goes through this file
from mosox import _mosox
from mosox._mosox import MosoxError, __version__

__all__ = ["Model", "MosoxError", "Solution", "Table", "__version__"]

Table = pd.DataFrame | pd.Series | pa.Table
"""Data for one set or param. Columns are read by position:
- param: the index columns, then the value
- set: the index columns (if indexed), then one column per member dimension

A pandas index (other than the default range index) is treated as the leading columns."""

Option = str | int | float | bool


@dataclass(frozen=True)
class Solution:
    objective: float
    variables: dict[str, pd.DataFrame]
    constraints: dict[str, pd.DataFrame]

    def __getitem__(self, name: str) -> pd.DataFrame:
        """A variable or constraint, with its index columns then `value` and `marginal`."""
        if name in self.variables:
            return self.variables[name]
        return self.constraints[name]


class Model:
    """A GMPL model, parsed once and compiled or solved against any data.

    Data given here overrides any in the model file itself."""

    def __init__(self, text: str) -> None:
        self._model = _mosox.Model(text)

    @classmethod
    def from_file(cls, path: str | PathLike[str]) -> Self:
        model = cls.__new__(cls)
        model._model = _mosox.Model.from_file(str(path))
        return model

    def to_mps(self, data: Mapping[str, Table] | None = None, *, prune: bool = True, check: bool = True) -> str:
        return self._model.to_mps(_to_arrow(data), prune, check)

    def solve(
        self,
        data: Mapping[str, Table] | None = None,
        *,
        prune: bool = True,
        check: bool = True,
        options: Mapping[str, Option] | None = None,
        verbose: bool = False,
    ) -> Solution:
        """Solve with HiGHS. `options` are HiGHS options, eg `{"time_limit": 60}`."""
        opts = [(k, str(v).lower() if isinstance(v, bool) else str(v)) for k, v in (options or {}).items()]
        objective, variables, constraints = self._model.solve(_to_arrow(data), prune, check, opts, verbose)
        return Solution(
            objective=objective,
            variables={name: t.to_pandas() for name, t in variables},
            constraints={name: t.to_pandas() for name, t in constraints},
        )


def _to_arrow(data: Mapping[str, Table] | None) -> dict[str, pa.Table]:
    return {name: _table(t) for name, t in (data or {}).items()}


def _table(t: Table) -> pa.Table:
    if isinstance(t, pa.Table):
        return t
    df = t.to_frame() if isinstance(t, pd.Series) else t
    if not (isinstance(df.index, pd.RangeIndex) and df.index.name is None):
        df = df.reset_index()
    return pa.Table.from_pandas(df, preserve_index=False)
