"""Python bindings for mosox, an LP matrix generator for GMPL."""

# The Rust bindings are in `_mosox`, wrapped by the modules here
from mosox._data import Data, Scalar, Table, Value
from mosox._model import Declaration, Matrix, Model, Option
from mosox._mosox import MosoxError, __version__
from mosox._run import (
    Compiled,
    Event,
    Failed,
    Finished,
    Label,
    Results,
    Run,
    Scenario,
    Skipped,
    Started,
    product,
)
from mosox._solution import Format, Solution, Stats

__all__ = [
    "Compiled",
    "Data",
    "Declaration",
    "Event",
    "Failed",
    "Finished",
    "Format",
    "Label",
    "Matrix",
    "Model",
    "MosoxError",
    "Option",
    "Results",
    "Run",
    "Scalar",
    "Scenario",
    "Skipped",
    "Solution",
    "Started",
    "Stats",
    "Table",
    "Value",
    "__version__",
    "product",
]
