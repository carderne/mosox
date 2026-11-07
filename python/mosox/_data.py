from collections.abc import Iterable, Mapping
from numbers import Number
from os import PathLike
from typing import Any, Self

import pandas as pd
import pyarrow as pa

from mosox import _mosox

Table = pd.DataFrame | pd.Series | pa.Table
"""A table of data for one set or param. Columns are read by position:
- param: the index columns, then the value
- set: the index columns (if indexed), then one column per member dimension

Except that a param's index columns are matched by name if each is named for a different
index position, by its set or dummy (`REGION` or `r` in `{r in REGION}`), with one other
column for the value.

A pandas index (other than the default range index) is treated as the leading columns."""

Scalar = str | int | float

Value = Table | Scalar | Mapping[Any, Scalar] | Iterable[Any]
"""Data for one set or param:
- a `Table`
- a scalar: the value of a scalar param, or of every element of an indexed param
- a mapping: param values by index, with tuple keys for multiple dimensions
- any other iterable: set members, as tuples for multiple dimensions"""

# A parsed `.dat` file, or tables with param defaults and whether they patch lower layers
_Layer = _mosox.Dat | tuple[dict[str, pa.Table], dict[str, float], bool]


class Data:
    """Set and param data, in layers that each override those before them.

    The bottom layer is the model file's own `data;` section, if it has one.
    Data in a layer replaces any for the same set or param in lower layers,
    except where it patches them (see `update`).
    A param's default carries up through the layers unless one gives another.

    Data is immutable, so a base can be shared between scenarios that each update it."""

    _layers: tuple[_Layer, ...]

    def __init__(self, data: Mapping[str, Value] | None = None, *, defaults: Mapping[str, float] | None = None) -> None:
        self._layers = _layer(data, defaults, patch=False)

    @classmethod
    def from_dat(cls, path: str | PathLike[str]) -> Self:
        """Data from a GMPL `.dat` file, parsed once however many times it's used"""
        return cls._of((_mosox.Dat.from_file(str(path)),))

    @classmethod
    def from_otoole(cls, data: Mapping[str, pd.DataFrame], defaults: Mapping[str, float] | None = None) -> Self:
        """Data as read by otoole, eg `Data.from_otoole(*otoole.read(config, "csv", path))`.

        Skips otoole's internal sets (like `_REGION`) and defaults for anything without data (like results)."""
        data = {name: df for name, df in data.items() if not name.startswith("_")}
        defaults = {name: value for name, value in (defaults or {}).items() if name in data}
        return cls(data, defaults=defaults)

    def update(
        self, data: "Data | Mapping[str, Value] | None" = None, *, defaults: Mapping[str, float] | None = None
    ) -> Self:
        """A new layer that patches params: rows are added to, or override, those below.
        Sets, and params given a single value for every element, are replaced.

        Given `Data`, its layers are added on top, each as it was built."""
        if isinstance(data, Data):
            return self._of((*self._layers, *data._layers, *_layer(None, defaults, patch=True)))
        return self._of((*self._layers, *_layer(data, defaults, patch=True)))

    def replace(self, data: Mapping[str, Value] | None = None, *, defaults: Mapping[str, float] | None = None) -> Self:
        """A new layer that replaces any data below for the same sets and params"""
        return self._of((*self._layers, *_layer(data, defaults, patch=False)))

    def __repr__(self) -> str:
        return f"Data({len(self._layers)} layers)"

    @classmethod
    def _of(cls, layers: tuple[_Layer, ...]) -> Self:
        data = cls.__new__(cls)
        data._layers = layers
        return data


def _layers(data: Data | Mapping[str, Value] | None) -> list[_Layer]:
    if isinstance(data, Data):
        return list(data._layers)
    return list(_layer(data, None, patch=False))


def _layer(
    data: Mapping[str, Value] | None, defaults: Mapping[str, float] | None, *, patch: bool
) -> tuple[_Layer, ...]:
    if not data and not defaults:
        return ()
    tables = {name: _table(value) for name, value in (data or {}).items()}
    return ((tables, {name: float(value) for name, value in (defaults or {}).items()}, patch),)


def _table(value: Value) -> pa.Table:
    if isinstance(value, pa.Table):
        return value
    if isinstance(value, pd.DataFrame | pd.Series):
        df = value.to_frame() if isinstance(value, pd.Series) else value
        if not (isinstance(df.index, pd.RangeIndex) and df.index.name is None):
            df = df.reset_index()
        return pa.Table.from_pandas(df, preserve_index=False)
    if isinstance(value, str | Number):
        return pa.table({"VALUE": [value]})
    if isinstance(value, Mapping):
        keys = [key if isinstance(key, tuple) else (key,) for key in value]
        columns = list(zip(*keys, strict=True)) if keys else [()]
        return pa.table({**{f"i{i}": list(col) for i, col in enumerate(columns)}, "VALUE": list(value.values())})
    members = [m if isinstance(m, tuple) else (m,) for m in value]
    columns = list(zip(*members, strict=True)) if members else [()]
    return pa.table({f"c{i}": list(col) for i, col in enumerate(columns)})
