use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, Int64Array, RecordBatch, StringArray};
use mosox::normalize::{compare_mps, normalize_mps};
use mosox::{GenOptions, Model, TableSource, matrix_to_mps_string};

const OPTS: GenOptions = GenOptions {
    prune: true,
    check: true,
};

/// The transport example without its `data;` section
fn transport() -> Model {
    let text = std::fs::read_to_string("examples/2d_params/model.mod").unwrap();
    Model::parse(text.split("data;").next().unwrap()).unwrap()
}

fn strs(vals: &[&str]) -> ArrayRef {
    Arc::new(StringArray::from_iter_values(vals))
}

fn table(cols: Vec<ArrayRef>) -> RecordBatch {
    RecordBatch::try_from_iter(
        cols.into_iter()
            .enumerate()
            .map(|(i, c)| (format!("c{i}"), c)),
    )
    .unwrap()
}

fn transport_data(cost_p1_w1: i64) -> TableSource {
    let mut data = TableSource::new();
    data.add("PLANTS", table(vec![strs(&["P1", "P2"])]))
        .add("WAREHOUSES", table(vec![strs(&["W1", "W2", "W3"])]))
        .add(
            "supply",
            table(vec![
                strs(&["P1", "P2"]),
                Arc::new(Int64Array::from(vec![100, 150])),
            ]),
        )
        .add(
            "demand",
            table(vec![
                strs(&["W1", "W2", "W3"]),
                Arc::new(Float64Array::from(vec![80.0, 70.0, 50.0])),
            ]),
        )
        .add(
            "cost",
            table(vec![
                strs(&["P1", "P1", "P1", "P2", "P2", "P2"]),
                strs(&["W1", "W2", "W3", "W1", "W2", "W3"]),
                Arc::new(Int64Array::from(vec![cost_p1_w1, 15, 20, 12, 8, 14])),
            ]),
        );
    data
}

fn compile(model: &Model, data: TableSource) -> anyhow::Result<String> {
    Ok(matrix_to_mps_string(&model.compile(data, &OPTS)?, "model"))
}

#[test]
fn transport_from_tables_matches_reference() {
    let mps = compile(&transport(), transport_data(10)).unwrap();
    let mut normalized = vec![];
    normalize_mps(mps.as_bytes(), &mut normalized);
    let expected = std::fs::read("examples/2d_params/matrix.mps").unwrap();
    let diffs = compare_mps(expected.as_slice(), normalized.as_slice(), 0.0);
    assert!(diffs.is_empty(), "{diffs:#?}");
}

#[test]
fn scenario_loop_reuses_model() {
    let model = transport();
    for cost in [10, 99, 10] {
        let mps = compile(&model, transport_data(cost)).unwrap();
        assert!(
            mps.contains(&format!(" ship[P1,W1] total_cost {cost}\n")),
            "{mps}"
        );
    }
}

#[test]
fn null_values_use_default() {
    let mut data = TableSource::new();
    data.add("PLANTS", table(vec![strs(&["P1", "P2"])]))
        .add("WAREHOUSES", table(vec![strs(&["W1"])]))
        .add(
            "supply",
            table(vec![
                strs(&["P1", "P2"]),
                Arc::new(Int64Array::from(vec![Some(100), None])),
            ]),
        )
        .set_default("supply", 5.0)
        .set_default("demand", 0.0)
        .set_default("cost", 1.0);
    let mps = compile(&transport(), data).unwrap();
    assert!(mps.contains(" RHS1 supply_limit[P1] 100\n"), "{mps}");
    assert!(mps.contains(" RHS1 supply_limit[P2] 5\n"), "{mps}");
}

#[test]
fn table_errors() {
    let model = transport();
    let cases: Vec<(TableSource, &str)> = vec![
        (
            {
                let mut d = transport_data(10);
                d.add("nope", table(vec![strs(&["x"])]));
                d
            },
            "Data for 'nope' has no matching model declaration",
        ),
        (
            {
                let mut d = transport_data(10);
                d.add(
                    "WAREHOUSES",
                    table(vec![Arc::new(Float64Array::from(vec![1.5]))]),
                );
                d
            },
            "1.5 is not a valid set element",
        ),
        (
            {
                let mut d = transport_data(10);
                d.add("demand", table(vec![strs(&["W1"])]));
                d
            },
            "batches have different numbers of columns",
        ),
    ];
    for (data, expected) in cases {
        let err = format!("{:#}", compile(&model, data).unwrap_err());
        assert!(err.contains(expected), "expected {expected:?} in {err:?}");
    }
}
