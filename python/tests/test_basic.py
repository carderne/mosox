from pathlib import Path

import pandas as pd
import pytest

import mosox

TRANSPORT = Path(__file__).parents[2] / "examples/2d_params/model.mod"


def transport_data(cost_p1_w1: float = 10) -> dict[str, pd.DataFrame]:
    return {
        "PLANTS": pd.DataFrame({"plant": ["P1", "P2"]}),
        "WAREHOUSES": pd.DataFrame({"warehouse": ["W1", "W2", "W3"]}),
        "supply": pd.DataFrame({"plant": ["P1", "P2"], "value": [100, 150]}),
        "demand": pd.DataFrame({"warehouse": ["W1", "W2", "W3"], "value": [80, 70, 50]}),
        "cost": pd.DataFrame(
            {
                "plant": ["P1"] * 3 + ["P2"] * 3,
                "warehouse": ["W1", "W2", "W3"] * 2,
                "value": [cost_p1_w1, 15, 20, 12, 8, 14],
            }
        ),
    }


def test_solve() -> None:
    model = mosox.Model.from_file(str(TRANSPORT))
    result = model.solve(transport_data())
    assert result.status == "optimal" and result.optimal
    # P1 supplies W1; P2 supplies W2 and W3, where it is cheaper
    assert result.objective == 10 * 80 + 8 * 70 + 14 * 50
    ship = result["ship"]
    assert list(ship.columns) == ["PLANTS", "WAREHOUSES", "VALUE", "MARGINAL"]
    assert ship.set_index(["PLANTS", "WAREHOUSES"]).loc[("P2", "W2"), "VALUE"] == 70
    assert list(result.constraints["supply_limit"].columns) == ["PLANTS", "VALUE", "MARGINAL"]
    assert result.stats.num_cols == 6 and result.stats.solve_seconds >= 0


def test_scenario_loop() -> None:
    model = mosox.Model.from_file(TRANSPORT)
    # At 20, P2 supplies W1 and P1 is left supplying W3
    objectives = [model.solve(transport_data(cost)).objective for cost in [10, 11, 20]]
    assert objectives == [10 * 80 + 8 * 70 + 14 * 50, 11 * 80 + 8 * 70 + 14 * 50, 12 * 80 + 8 * 70 + 20 * 50]


def test_to_mps() -> None:
    mps = mosox.Model.from_file(TRANSPORT).to_mps(transport_data(99))
    assert " ship[P1,W1] total_cost 99\n" in mps


def test_compile() -> None:
    matrix = mosox.Model.from_file(TRANSPORT).compile(transport_data())
    assert (matrix.num_rows, matrix.num_cols, matrix.num_nonzeros) == (6, 6, 18)
    # A matrix can be solved more than once
    assert matrix.solve().objective == matrix.solve(options={"presolve": "off"}).objective == 2060


def test_indexed_series() -> None:
    cost = transport_data()["cost"].set_index(["plant", "warehouse"])["value"]
    data: dict[str, mosox.Value] = {**transport_data(), "cost": cost}
    assert mosox.Model.from_file(TRANSPORT).solve(data).objective == 2060


def test_options() -> None:
    model = mosox.Model.from_file(TRANSPORT)
    assert model.solve(transport_data(), options={"presolve": "off", "threads": 1}).objective == 2060
    with pytest.raises(mosox.MosoxError, match="unknown option 'nope'"):
        model.solve(transport_data(), options={"nope": 1})


def test_keep() -> None:
    model = mosox.Model.from_file(TRANSPORT)
    solution = model.solve(transport_data(), keep=["ship"])
    assert list(solution.variables) == ["ship"] and not solution.constraints
    with pytest.raises(ValueError, match="no variables or constraints named nope"):
        model.solve(transport_data(), keep=["nope"])


def test_declarations() -> None:
    model = mosox.Model.from_file(TRANSPORT)
    assert list(model.sets) == ["PLANTS", "WAREHOUSES"]
    assert model.params["cost"] == mosox.Declaration("cost", "param", ("PLANTS", "WAREHOUSES"))
    assert model.variables["ship"].index == ("PLANTS", "WAREHOUSES")
    assert list(model.constraints) == ["supply_limit", "meet_demand"]
    assert model.objective == "total_cost"


def test_column_names() -> None:
    # A repeated set falls back to dummy names, and a multi-dimensional set to positions
    model = mosox.Model(
        """
        set A; set AB dimen 2;
        var x{a in A, b in A} >= 0;
        var y{AB} >= 0;
        minimize z: sum{a in A, b in A} x[a,b] + sum{(a,b) in AB} y[a,b];
        """
    )
    assert model.variables["x"].index == ("a", "b")
    assert model.variables["y"].index == ("i0", "i1")
    assert model.sets["AB"].dimen == 2
    solution = model.solve({"A": ["a1"], "AB": [("a1", 2)]})
    assert list(solution["y"].columns) == ["i0", "i1", "VALUE", "MARGINAL"]


def test_infeasible() -> None:
    data = transport_data()
    data["supply"] = pd.DataFrame({"plant": ["P1", "P2"], "value": [1, 1]})
    solution = mosox.Model.from_file(TRANSPORT).solve(data)
    assert solution.status == "infeasible" and not solution.optimal
    assert pd.isna(solution.objective)
    assert not solution.variables and not solution.constraints


def test_errors() -> None:
    data = transport_data()
    data["supply"] = pd.DataFrame({"plant": ["P1", "P3"], "value": [100, 150]})
    with pytest.raises(mosox.MosoxError, match=r"supply\[P3\] is outside its domain"):
        mosox.Model.from_file(TRANSPORT).solve(data)
    with pytest.raises(mosox.MosoxError, match="Syntax error"):
        mosox.Model("set ;")
