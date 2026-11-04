"""Python bindings for mosox, an LP matrix generator for GMPL."""

# The Rust bindings are in `_mosox`, wrapped by the modules here
from mosox._data import Data, Scalar, Table, Value
from mosox._model import Declaration, Matrix, Model, Option
from mosox._mosox import MosoxError, __version__
from mosox._solution import Format, Solution, Stats

__all__ = [
    "Data",
    "Declaration",
    "Format",
    "Matrix",
    "Model",
    "MosoxError",
    "Option",
    "Scalar",
    "Solution",
    "Stats",
    "Table",
    "Value",
    "__version__",
]
