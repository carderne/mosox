from pathlib import Path

import pandas as pd
import pyarrow as pa
import pytest

import mosox

EXAMPLES = Path(__file__).parents[2] / "examples"
# With its own `data;` section: cost P1 W1-W3 = 10 15 20, P2 = 12 8 14
TRANSPORT = mosox.Model.from_file(EXAMPLES / "2d_params/model.mod")


def cost(mps: str, plant: str, warehouse: str) -> float:
    prefix = f" ship[{plant},{warehouse}] total_cost "
    return float(next(line for line in mps.splitlines() if line.startswith(prefix)).removeprefix(prefix))


def test_python_values() -> None:
    data = {
        "PLANTS": ["P1", "P2"],
        "WAREHOUSES": ("W1", "W2", "W3"),
        "supply": {"P1": 100, "P2": 150},
        "demand": pd.Series([80, 70, 50], index=pd.Index(["W1", "W2", "W3"], name="WAREHOUSES")),
        "cost": {(p, w): 1 for p in ["P1", "P2"] for w in ["W1", "W2", "W3"]} | {("P1", "W1"): 5},
    }
    mps = TRANSPORT.to_mps(data)
    assert cost(mps, "P1", "W1") == 5 and cost(mps, "P2", "W3") == 1


def test_scalar_fills_indexed_param() -> None:
    mps = TRANSPORT.to_mps({"cost": 3})
    assert cost(mps, "P1", "W1") == cost(mps, "P2", "W3") == 3
    with pytest.raises(mosox.MosoxError, match="both a value for every index and a default"):
        TRANSPORT.to_mps(mosox.Data({"cost": 3}, defaults={"cost": 1}))


def test_columns_by_name() -> None:
    # Set names in any order, with the value column anywhere
    cost_ = pd.DataFrame({"VALUE": [99], "WAREHOUSES": ["W1"], "PLANTS": ["P1"]})
    mps = TRANSPORT.to_mps(mosox.Data().update({"cost": cost_}))
    assert cost(mps, "P1", "W1") == 99 and cost(mps, "P1", "W2") == 15


def test_mapping_replaces_model_data() -> None:
    mps = TRANSPORT.to_mps(mosox.Data({"cost": {("P1", "W1"): 99}}, defaults={"cost": 1}))
    assert cost(mps, "P1", "W1") == 99 and cost(mps, "P1", "W2") == 1


def test_update_patches() -> None:
    base = mosox.Data({"cost": {("P1", "W1"): 99}}, defaults={"cost": 1})
    patched = base.update({"cost": {("P2", "W3"): 50}})
    mps = TRANSPORT.to_mps(patched)
    assert (cost(mps, "P1", "W1"), cost(mps, "P2", "W3"), cost(mps, "P1", "W2")) == (99, 50, 1)
    # The base is unchanged
    assert cost(TRANSPORT.to_mps(base), "P2", "W3") == 1
    # Only a default, over the model file's data
    mps = TRANSPORT.to_mps(mosox.Data().update({"cost": {("P1", "W1"): 7}}, defaults={"cost": 2}))
    assert (cost(mps, "P1", "W1"), cost(mps, "P1", "W2")) == (7, 15)


def test_replace() -> None:
    base = mosox.Data().update({"cost": {("P1", "W1"): 99}})
    mps = TRANSPORT.to_mps(base.replace({"cost": {("P2", "W3"): 50}}, defaults={"cost": 4}))
    assert (cost(mps, "P1", "W1"), cost(mps, "P2", "W3")) == (4, 50)


def test_update_with_data() -> None:
    overlay = mosox.Data().update({"cost": {("P1", "W1"): 1}}).update({"cost": {("P1", "W2"): 2}})
    mps = TRANSPORT.to_mps(mosox.Data().update(overlay))
    assert (cost(mps, "P1", "W1"), cost(mps, "P1", "W2"), cost(mps, "P1", "W3")) == (1, 2, 20)


def test_sets_are_replaced() -> None:
    data = mosox.Data().update({"WAREHOUSES": ["W1"]})
    # Patched params keep the model file's data for the removed warehouses
    with pytest.raises(mosox.MosoxError, match=r"demand\[W2\] is outside its domain"):
        TRANSPORT.solve(data)
    data = data.replace({"demand": {"W1": 80}, "cost": {("P1", "W1"): 10, ("P2", "W1"): 12}})
    assert TRANSPORT.solve(data).objective == 10 * 80


def test_from_dat(tmp_path: Path) -> None:
    model = mosox.Model.from_file(EXAMPLES / "osemosys_small/osemosys.mod")
    base = mosox.Data.from_dat(EXAMPLES / "osemosys_small/ose_small.dat")
    solution = model.solve(base)
    assert solution.optimal
    assert list(solution["NewCapacity"].columns) == ["REGION", "TECHNOLOGY", "YEAR", "VALUE", "MARGINAL"]
    # Reusing the parsed base, patched
    discounted = model.solve(base.update({"DiscountRate": 0.5}))
    assert discounted.objective < solution.objective


def test_from_otoole() -> None:
    # As returned by `otoole.read`: frames indexed by set names, with VALUE columns;
    # an internal `_REGION` set; and defaults for everything, including results
    data = {
        "PLANTS": pd.DataFrame({"VALUE": ["P1", "P2"]}),
        "_PLANTS": pd.DataFrame({"VALUE": ["P1", "P2"]}),
        "WAREHOUSES": pd.DataFrame({"VALUE": ["W1", "W2", "W3"]}),
        "supply": pd.DataFrame({"PLANTS": ["P1", "P2"], "VALUE": [100, 150]}).set_index("PLANTS"),
        "demand": pd.DataFrame({"WAREHOUSES": ["W1", "W2", "W3"], "VALUE": [80, 70, 50]}).set_index("WAREHOUSES"),
        "cost": pd.DataFrame(columns=["PLANTS", "WAREHOUSES", "VALUE"]).set_index(["PLANTS", "WAREHOUSES"]),
    }
    defaults = {"supply": 0, "demand": 0, "cost": 1, "ship": 0}
    solution = TRANSPORT.solve(mosox.Data.from_otoole(data, defaults))
    assert solution.objective == 200


def test_arrow_and_errors() -> None:
    supply = pa.table({"p": ["P1", "P2"], "v": [100, 150]})
    assert TRANSPORT.solve({"supply": supply}).optimal
    with pytest.raises(mosox.MosoxError, match="Data for 'nope' has no matching model declaration"):
        TRANSPORT.solve({"nope": 1})
    with pytest.raises(ValueError):
        TRANSPORT.solve({"cost": {("P1", "W1"): 1, ("P2",): 2}})
