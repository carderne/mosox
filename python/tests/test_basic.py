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
    # P1 supplies W1; P2 supplies W2 and W3, where it is cheaper
    assert result.objective == 10 * 80 + 8 * 70 + 14 * 50
    ship = result["ship"]
    assert list(ship.columns) == ["p", "w", "value", "marginal"]
    assert ship.set_index(["p", "w"]).loc[("P2", "W2"), "value"] == 70


def test_scenario_loop() -> None:
    model = mosox.Model.from_file(TRANSPORT)
    # At 20, P2 supplies W1 and P1 is left supplying W3
    objectives = [model.solve(transport_data(cost)).objective for cost in [10, 11, 20]]
    assert objectives == [10 * 80 + 8 * 70 + 14 * 50, 11 * 80 + 8 * 70 + 14 * 50, 12 * 80 + 8 * 70 + 20 * 50]


def test_to_mps() -> None:
    mps = mosox.Model.from_file(TRANSPORT).to_mps(transport_data(99))
    assert " ship[P1,W1] total_cost 99\n" in mps


def test_indexed_series() -> None:
    cost = transport_data()["cost"].set_index(["plant", "warehouse"])["value"]
    data: dict[str, mosox.Table] = {**transport_data(), "cost": cost}
    assert mosox.Model.from_file(TRANSPORT).solve(data).objective == 2060


def test_options() -> None:
    model = mosox.Model.from_file(TRANSPORT)
    assert model.solve(transport_data(), options={"presolve": "off", "threads": 1}).objective == 2060
    with pytest.raises(mosox.MosoxError, match="unknown option 'nope'"):
        model.solve(transport_data(), options={"nope": 1})


def test_errors() -> None:
    data = transport_data()
    data["supply"] = pd.DataFrame({"plant": ["P1", "P3"], "value": [100, 150]})
    with pytest.raises(mosox.MosoxError, match=r"supply\[P3\] is outside its domain"):
        mosox.Model.from_file(TRANSPORT).solve(data)
    with pytest.raises(mosox.MosoxError, match="Syntax error"):
        mosox.Model("set ;")
