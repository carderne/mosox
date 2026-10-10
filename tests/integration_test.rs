use mosox::Model;

#[test]
fn test_load() {
    Model::from_file("examples/osemosys_small/osemosys.mod").unwrap();
}
