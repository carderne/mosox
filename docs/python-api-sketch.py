import mosox, otoole

model = mosox.Model.from_file("osemosys.mod")
base = mosox.Data.from_dat("utopia.dat")
# or: base = mosox.Data.from_otoole(*otoole.read("config.yaml", "csv", "data/"))

data = base.update({
    "DiscountRate": 0.08,                                 # a single value fills every index
    "CapitalCost": {("UTOPIA", "E01", 2030): 900},       # patch individual rows
    "ResidualCapacity": residual_df,                      # DataFrame: REGION, TECHNOLOGY, YEAR, VALUE
})
sol = model.solve(data)
sol.status, sol.objective
sol["NewCapacity"]                                        # REGION, TECHNOLOGY, YEAR, VALUE, MARGINAL

# Ensemble: every combination of factors, in parallel
scenarios = mosox.product(
    co2={"none": {}, "high": {"EmissionsPenalty": 100}},
    discount={0.05: {}, 0.10: {"DiscountRate": 0.10}},
)
run = model.run(scenarios, base, workers=4, keep=["NewCapacity"], store="results/")
for event in run:                                         # Started / Finished / Failed ...
    print(f"{event.done}/{event.total}", event.scenario.name)

results = run.results()                                   # or mosox.Results.open("results/")
results.summary                                           # one row per scenario: status, objective, timings
results["NewCapacity"]                                    # all scenarios, with scenario/co2/discount columns
