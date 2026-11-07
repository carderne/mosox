import asyncio
import itertools
import math
import queue
import threading
from collections.abc import AsyncIterator, Callable, Iterator, Mapping, Sequence
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from functools import partial
from os import PathLike
from pathlib import Path
from typing import TYPE_CHECKING, Any, Literal

import pandas as pd

from mosox._data import Data, Value
from mosox._solution import Solution, Stats, _read_meta, _write

if TYPE_CHECKING:
    from mosox._model import Matrix

Label = str | int | float
"""A scenario's position along one dimension of a sweep, eg `"high"` or `0.05`"""


@dataclass(frozen=True)
class Scenario:
    name: str
    """Unique within a run, and used as its directory name in a store"""
    data: Data | Mapping[str, Value] = field(default_factory=dict)
    """Updates to the run's base data, as by `Data.update`"""
    coords: Mapping[str, Label] = field(default_factory=dict)
    """Labels for the scenario, as columns in results, eg `{"carbon": "high"}`"""
    options: Mapping[str, str | int | float | bool] = field(default_factory=dict)
    """HiGHS options, over the run's"""


def product(**factors: Mapping[Label, Data | Mapping[str, Value]]) -> list[Scenario]:
    """A scenario for each combination of the factors' levels, applying each level's updates in turn.

    ```
    product(carbon={"low": {"EmissionsPenalty": 10}, "high": {"EmissionsPenalty": 100}},
            discount={0.03: {"DiscountRate": 0.03}, 0.07: {"DiscountRate": 0.07}})
    ```
    gives four scenarios, named like `carbon=high,discount=0.03`."""
    scenarios = []
    for levels in itertools.product(*(factor.items() for factor in factors.values())):
        coords = {name: label for name, (label, _) in zip(factors, levels, strict=True)}
        data = Data()
        for _, updates in levels:
            data = data.update(updates)
        name = ",".join(f"{k}={v}" for k, v in coords.items())
        scenarios.append(Scenario(name, data, coords))
    return scenarios


@dataclass(frozen=True)
class Started:
    scenario: Scenario
    done: int
    """Scenarios finished, failed or skipped so far"""
    total: int


@dataclass(frozen=True)
class Compiled:
    scenario: Scenario
    num_rows: int
    num_cols: int
    num_nonzeros: int
    seconds: float
    done: int
    total: int


@dataclass(frozen=True)
class Finished:
    """Solved, though not necessarily to optimality: see `solution.status`"""

    scenario: Scenario
    solution: Solution
    done: int
    total: int


@dataclass(frozen=True)
class Failed:
    scenario: Scenario
    error: Exception
    done: int
    total: int


@dataclass(frozen=True)
class Skipped:
    scenario: Scenario
    reason: Literal["cancelled", "stored"]
    done: int
    total: int


Event = Started | Compiled | Finished | Failed | Skipped

_DONE = (Finished, Failed, Skipped)


@dataclass(frozen=True)
class _Entry:
    """One scenario's outcome, with its solution in memory or in a store (unless it failed)"""

    scenario: str
    coords: Mapping[str, Label]
    status: str
    objective: float
    error: str | None
    stats: Stats | None
    solution: Solution | Path | None

    @classmethod
    def from_meta(cls, path: Path, meta: dict[str, Any]) -> "_Entry":
        failed = meta["status"] == "error"
        return cls(
            scenario=meta["scenario"],
            coords=meta["coords"],
            status=meta["status"],
            objective=math.nan if meta.get("objective") is None else meta["objective"],
            error=meta.get("error"),
            stats=None if failed else Stats(**meta["stats"]),
            solution=None if failed else path,
        )

    def load(self) -> Solution | None:
        return Solution.read(self.solution) if isinstance(self.solution, Path) else self.solution


class Run:
    """Scenarios being solved, from `Model.run`.

    Iterate (or async iterate) for events as they happen, or call `results()` to wait for them all.
    Either starts the run, which runs once, carrying on in the background if iteration stops:
    use `cancel()` to stop it."""

    def __init__(
        self,
        compiler: "Callable[[Data], Matrix]",
        solver: "Callable[[Matrix, Mapping[str, Any]], Solution]",
        scenarios: Sequence[Scenario],
        base: Data,
        *,
        workers: int,
        store: str | PathLike[str] | None,
        resume: bool,
    ) -> None:
        names = [s.name for s in scenarios]
        if duplicates := sorted({n for n in names if names.count(n) > 1}):
            raise ValueError(f"scenario names must be unique: {', '.join(duplicates)}")
        if store is not None and (bad := [n for n in names if not _dir_name(n)]):
            raise ValueError(f"scenario names can't be used as directory names: {', '.join(bad)}")
        self.scenarios = list(scenarios)
        self.total = len(scenarios)
        self._compile = compiler
        self._solve = solver
        self._base = base
        self._workers = workers
        self._store = None if store is None else Path(store)
        self._resume = resume
        self._entries: dict[str, _Entry] = {}
        self._cancelled = threading.Event()
        self._events: Iterator[Event] | None = None

    def __iter__(self) -> Iterator[Event]:
        if self._events is None:
            self._events = self._execute()
        return self._events

    async def __aiter__(self) -> AsyncIterator[Event]:
        events = iter(self)
        while (event := await asyncio.to_thread(_next, events)) is not None:
            yield event

    def cancel(self) -> None:
        """Skip scenarios not yet started. Those running finish."""
        self._cancelled.set()

    def results(self) -> "Results":
        """Wait for the run to end, then its results, in scenario order"""
        for _ in self:
            pass
        return Results([self._entries[s.name] for s in self.scenarios if s.name in self._entries])

    def _execute(self) -> Iterator[Event]:
        events: queue.Queue[Callable[..., Event]] = queue.Queue()
        pool = ThreadPoolExecutor(self._workers, thread_name_prefix="mosox")
        try:
            for scenario in self.scenarios:
                pool.submit(self._one, scenario, events.put)
            done = 0
            while done < self.total:
                try:
                    make = events.get(timeout=0.1)
                except queue.Empty:
                    continue
                assert isinstance(make, partial)
                if make.func in _DONE:
                    done += 1
                yield make(done=done, total=self.total)
        finally:
            # Also reached if the caller stops iterating early
            self._cancelled.set()
            pool.shutdown(cancel_futures=True)

    def _one(self, scenario: Scenario, emit: Callable[[Callable[..., Event]], None]) -> None:
        try:
            self._attempt(scenario, emit)
        except Exception as e:
            # Anything not already reported, eg from the store, so the run still ends
            self._entries[scenario.name] = _Entry(scenario.name, scenario.coords, "error", math.nan, str(e), None, None)
            emit(partial(Failed, scenario, e))

    def _attempt(self, scenario: Scenario, emit: Callable[[Callable[..., Event]], None]) -> None:
        if self._cancelled.is_set():
            return emit(partial(Skipped, scenario, "cancelled"))
        path = None if self._store is None else self._store / scenario.name
        if path is not None and self._resume and (meta := _read_meta(path)) and meta["status"] != "error":
            self._entries[scenario.name] = _Entry.from_meta(path, meta)
            return emit(partial(Skipped, scenario, "stored"))
        emit(partial(Started, scenario))
        extra = {"scenario": scenario.name, "coords": dict(scenario.coords)}
        try:
            matrix = self._compile(self._base.update(scenario.data))
            emit(
                partial(
                    Compiled, scenario, matrix.num_rows, matrix.num_cols, matrix.num_nonzeros, matrix.compile_seconds
                )
            )
            solution = self._solve(matrix, scenario.options)
            if path is not None:
                _write(path, solution, "parquet", extra)
        except Exception as e:
            self._entries[scenario.name] = _Entry(scenario.name, scenario.coords, "error", math.nan, str(e), None, None)
            if path is not None:
                _write(path, None, "parquet", {**extra, "status": "error", "error": str(e)})
            return emit(partial(Failed, scenario, e))
        self._entries[scenario.name] = _Entry(
            scenario=scenario.name,
            coords=scenario.coords,
            status=solution.status,
            objective=solution.objective,
            error=None,
            stats=solution.stats,
            solution=solution if path is None else path,
        )
        emit(partial(Finished, scenario, solution))


class Results:
    """The outcomes of a run's scenarios, from `Run.results()` or a store"""

    def __init__(self, entries: Sequence[_Entry]) -> None:
        self._entries = {e.scenario: e for e in entries}

    @classmethod
    def open(cls, store: str | PathLike[str]) -> "Results":
        """Results from a run's store, by scenario name"""
        paths = sorted(p for p in Path(store).iterdir() if p.is_dir())
        return cls([_Entry.from_meta(p, meta) for p in paths if (meta := _read_meta(p)) is not None])

    @property
    def scenarios(self) -> list[str]:
        return list(self._entries)

    def __len__(self) -> int:
        return len(self._entries)

    @property
    def summary(self) -> pd.DataFrame:
        """A row per scenario: its name, coords, status, objective, any error, and stats"""
        rows = []
        for e in self._entries.values():
            stats = asdict(e.stats) if e.stats else {}
            row = {"scenario": e.scenario, **e.coords, "status": e.status, "objective": e.objective}
            rows.append({**row, "error": e.error, **stats})
        return pd.DataFrame(rows)

    def solution(self, scenario: str) -> Solution:
        solution = self._entries[scenario].load()
        if solution is None:
            raise KeyError(f"scenario {scenario!r} failed: {self._entries[scenario].error}")
        return solution

    def __getitem__(self, name: str) -> pd.DataFrame:
        """A variable or constraint across scenarios, with columns for the scenario name and coords first"""
        frames = []
        for e in self._entries.values():
            solution = e.load()
            if solution is None or not (name in solution.variables or name in solution.constraints):
                continue
            df = solution[name]
            labels = {"scenario": e.scenario, **e.coords}
            if clash := sorted(set(labels) & set(df.columns)):
                raise ValueError(f"scenario coords clash with columns of {name}: {', '.join(clash)}")
            frames.append(pd.concat([pd.DataFrame(labels, index=df.index), df], axis=1))
        if not frames:
            raise KeyError(f"no scenario has a solution for {name!r}")
        return pd.concat(frames, ignore_index=True)

    def __repr__(self) -> str:
        return f"Results({len(self)} scenarios)"


def _next(events: Iterator[Event]) -> Event | None:
    return next(events, None)


def _dir_name(name: str) -> bool:
    return name not in ("", ".", "..") and not any(c in name for c in "/\\")
