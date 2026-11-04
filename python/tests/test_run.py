import asyncio
import json
from pathlib import Path

import pytest

import mosox

TRANSPORT = mosox.Model.from_file(Path(__file__).parents[2] / "examples/2d_params/model.mod")

# Cost of P1 to W1, with demand at W1 from 80
SCENARIOS = mosox.product(
    cost={"low": {"cost": {("P1", "W1"): 10}}, "high": {"cost": {("P1", "W1"): 20}}},
    demand={80: {}, 90: {"demand": {"W1": 90}}},
)
# When P1 to W1 is dearer, P2 supplies W1 and P1 makes up the rest
OBJECTIVES = {
    ("low", 80): 10 * 80 + 8 * 70 + 14 * 50,
    ("low", 90): 10 * 90 + 8 * 70 + 14 * 50,
    ("high", 80): 12 * 80 + 8 * 70 + 20 * 50,
    ("high", 90): 12 * 90 + 8 * 60 + 15 * 10 + 20 * 50,
}


def test_product() -> None:
    assert [s.name for s in SCENARIOS] == [
        "cost=low,demand=80",
        "cost=low,demand=90",
        "cost=high,demand=80",
        "cost=high,demand=90",
    ]
    assert SCENARIOS[3].coords == {"cost": "high", "demand": 90}


def test_run() -> None:
    run = TRANSPORT.run(SCENARIOS, workers=2)
    events = list(run)
    kinds = [type(e).__name__ for e in events]
    assert kinds.count("Started") == kinds.count("Compiled") == kinds.count("Finished") == 4
    assert [e.done for e in events if isinstance(e, mosox.Finished)] == [1, 2, 3, 4]
    assert all(e.total == 4 for e in events)

    results = run.results()
    summary = results.summary
    assert list(summary.columns[:5]) == ["scenario", "cost", "demand", "status", "objective"]
    assert list(summary["objective"]) == list(OBJECTIVES.values())
    assert (summary["num_cols"] == 6).all()

    ship = results["ship"]
    assert list(ship.columns) == ["scenario", "cost", "demand", "PLANTS", "WAREHOUSES", "VALUE", "MARGINAL"]
    assert len(ship) == 4 * 6
    high = ship.query("cost == 'high' and demand == 90").set_index(["PLANTS", "WAREHOUSES"])
    assert high.loc[("P2", "W1"), "VALUE"] == 90
    assert results.solution("cost=low,demand=80").objective == OBJECTIVES["low", 80]


def test_results_without_iterating() -> None:
    results = TRANSPORT.run([mosox.Scenario("base")], keep=["ship"]).results()
    assert results.scenarios == ["base"]
    assert list(results.solution("base").variables) == ["ship"]
    assert results["ship"]["scenario"].unique().tolist() == ["base"]


def test_base_and_options() -> None:
    base = mosox.Data().update({"demand": {"W1": 90}})
    scenarios = [
        mosox.Scenario("base"),
        mosox.Scenario("data", {"demand": {"W1": 80}}),
        mosox.Scenario("bad option", options={"nope": 1}),
    ]
    results = TRANSPORT.run(scenarios, base).results()
    summary = results.summary.set_index("scenario")
    assert summary.loc["base", "objective"] == OBJECTIVES["low", 90]
    assert summary.loc["data", "objective"] == OBJECTIVES["low", 80]
    assert summary.loc["bad option", "status"] == "error"
    assert "unknown option 'nope'" in str(summary.loc["bad option", "error"])
    with pytest.raises(KeyError, match="failed"):
        results.solution("bad option")


def test_failures_and_infeasible() -> None:
    scenarios = [
        mosox.Scenario("bad", {"supply": {"P3": 1}}),
        mosox.Scenario("infeasible", {"supply": 1}),
    ]
    events = list(TRANSPORT.run(scenarios))
    failed = next(e for e in events if isinstance(e, mosox.Failed))
    assert isinstance(failed.error, mosox.MosoxError) and "outside its domain" in str(failed.error)
    finished = next(e for e in events if isinstance(e, mosox.Finished))
    assert finished.solution.status == "infeasible"


def test_store_and_resume(tmp_path: Path) -> None:
    scenarios = [*SCENARIOS, mosox.Scenario("bad", {"supply": {"P3": 1}})]
    results = TRANSPORT.run(scenarios, store=tmp_path, keep=["ship"]).results()
    assert sorted(p.name for p in tmp_path.iterdir()) == sorted(s.name for s in scenarios)
    meta = json.loads((tmp_path / "cost=high,demand=90/meta.json").read_text())
    assert meta["coords"] == {"cost": "high", "demand": 90} and meta["variables"] == ["ship"]
    assert json.loads((tmp_path / "bad/meta.json").read_text())["status"] == "error"

    opened = mosox.Results.open(tmp_path)
    assert sorted(opened.scenarios) == sorted(results.scenarios)
    assert len(opened["ship"]) == len(results["ship"]) == 4 * 6

    # Resuming skips stored scenarios, but retries failures
    events = list(TRANSPORT.run(scenarios, store=tmp_path, resume=True))
    skipped = [e.scenario.name for e in events if isinstance(e, mosox.Skipped)]
    assert sorted(skipped) == sorted(s.name for s in SCENARIOS)
    assert all(e.reason == "stored" for e in events if isinstance(e, mosox.Skipped))
    assert [e.scenario.name for e in events if isinstance(e, mosox.Failed)] == ["bad"]


def test_cancel() -> None:
    run = TRANSPORT.run(SCENARIOS)
    events = []
    for event in run:
        events.append(event)
        if isinstance(event, mosox.Finished):
            run.cancel()
    # Later scenarios may have started before the first finished event was seen
    skipped = [e for e in events if isinstance(e, mosox.Skipped)]
    assert all(e.reason == "cancelled" for e in skipped)
    assert len(run.results()) == 4 - len(skipped)


def test_stop_iterating() -> None:
    # The run carries on, to be picked up again
    run = TRANSPORT.run(SCENARIOS)
    for _ in run:
        break
    assert len(run.results()) == 4


def test_async() -> None:
    async def collect() -> list[mosox.Event]:
        return [e async for e in TRANSPORT.run(SCENARIOS, workers=4)]

    events = asyncio.run(collect())
    assert sum(isinstance(e, mosox.Finished) for e in events) == 4


def test_invalid_scenarios() -> None:
    with pytest.raises(ValueError, match="unique"):
        TRANSPORT.run([mosox.Scenario("a"), mosox.Scenario("a")])
    with pytest.raises(ValueError, match="directory names"):
        TRANSPORT.run([mosox.Scenario("a/b")], store="unused")
    with pytest.raises(ValueError, match="clash"):
        TRANSPORT.run([mosox.Scenario("a", coords={"PLANTS": 1})]).results()["ship"]
    with pytest.raises(ValueError, match="no variables or constraints named"):
        TRANSPORT.run([], keep=["nope"])
    assert mosox.Results([]).summary.empty
