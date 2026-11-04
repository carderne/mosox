from collections.abc import Collection, Mapping
from dataclasses import dataclass
from os import PathLike
from time import perf_counter
from typing import Literal, Self

from mosox import _mosox
from mosox._data import Data, Value, _layers
from mosox._solution import Solution, Stats

Option = str | int | float | bool
"""A HiGHS option value"""


@dataclass(frozen=True)
class Declaration:
    name: str
    kind: Literal["set", "param", "var", "constraint", "objective"]
    index: tuple[str, ...]
    """Index column names, as used in result tables and matched in param tables"""
    dimen: int | None = None
    """Member dimension, for sets"""


class Model:
    """A GMPL model, parsed once and compiled or solved against any data.

    Data can be given as `Data`, or a mapping of names to values, which replaces any data in the model file."""

    def __init__(self, text: str) -> None:
        self._init(_mosox.Model(text))

    @classmethod
    def from_file(cls, path: str | PathLike[str]) -> Self:
        model = cls.__new__(cls)
        model._init(_mosox.Model.from_file(str(path)))
        return model

    def _init(self, model: _mosox.Model) -> None:
        self._model = model
        self._decls = {
            name: Declaration(name, kind, tuple(index), dimen) for kind, name, index, dimen in model.declarations()
        }

    def _of(self, *kinds: str) -> dict[str, Declaration]:
        return {name: d for name, d in self._decls.items() if d.kind in kinds}

    @property
    def sets(self) -> dict[str, Declaration]:
        return self._of("set")

    @property
    def params(self) -> dict[str, Declaration]:
        return self._of("param")

    @property
    def variables(self) -> dict[str, Declaration]:
        return self._of("var")

    @property
    def constraints(self) -> dict[str, Declaration]:
        return self._of("constraint")

    @property
    def objective(self) -> str | None:
        return next(iter(self._of("objective")), None)

    def compile(
        self, data: Data | Mapping[str, Value] | None = None, *, prune: bool = True, check: bool = True
    ) -> "Matrix":
        """`prune` drops zero coefficients and empty columns.
        `check` enforces check statements and declared constraints on the data."""
        start = perf_counter()
        matrix = self._model.compile(_layers(data), prune, check)
        return Matrix(matrix, perf_counter() - start, self)

    def to_mps(self, data: Data | Mapping[str, Value] | None = None, *, prune: bool = True, check: bool = True) -> str:
        return self.compile(data, prune=prune, check=check).to_mps()

    def solve(
        self,
        data: Data | Mapping[str, Value] | None = None,
        *,
        prune: bool = True,
        check: bool = True,
        options: Mapping[str, Option] | None = None,
        verbose: bool = False,
        keep: Collection[str] | None = None,
    ) -> Solution:
        """Compile and solve with HiGHS. See `Matrix.solve`."""
        matrix = self.compile(data, prune=prune, check=check)
        return matrix.solve(options=options, verbose=verbose, keep=keep)

    def _check_keep(self, keep: Collection[str] | None) -> None:
        unknown = sorted(set(keep or ()) - set(self.variables) - set(self.constraints))
        if unknown:
            raise ValueError(f"no variables or constraints named {', '.join(unknown)}")


class Matrix:
    """A compiled model and data"""

    def __init__(self, matrix: _mosox.Matrix, compile_seconds: float, model: Model) -> None:
        self._matrix = matrix
        self._model = model
        self.compile_seconds = compile_seconds

    @property
    def num_rows(self) -> int:
        """Including the objective"""
        return self._matrix.num_rows

    @property
    def num_cols(self) -> int:
        return self._matrix.num_cols

    @property
    def num_nonzeros(self) -> int:
        return self._matrix.num_nonzeros

    def to_mps(self) -> str:
        return self._matrix.to_mps()

    def solve(
        self,
        *,
        options: Mapping[str, Option] | None = None,
        verbose: bool = False,
        keep: Collection[str] | None = None,
    ) -> Solution:
        """Solve with HiGHS. `options` are HiGHS options, eg `{"time_limit": 60}`.
        `keep` limits the solution to those variables and constraints."""
        self._model._check_keep(keep)
        opts = [(k, str(v).lower() if isinstance(v, bool) else str(v)) for k, v in (options or {}).items()]
        start = perf_counter()
        status, objective, variables, constraints = self._matrix.solve(
            opts, verbose, None if keep is None else set(keep)
        )
        stats = Stats(self.num_rows, self.num_cols, self.num_nonzeros, self.compile_seconds, perf_counter() - start)
        return Solution(status, objective, stats, dict(variables), dict(constraints))

    def __repr__(self) -> str:
        return f"Matrix({self.num_rows} rows, {self.num_cols} cols, {self.num_nonzeros} nonzeros)"
