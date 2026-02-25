use anyhow::{Result, anyhow};
use highs::Model;

/// The type of value a HiGHS option expects.
#[derive(Debug, Clone, Copy)]
enum OptionType {
    Bool,
    Int,
    Float,
    String,
}

// Full option reference:
// https://ergo-code.github.io/HiGHS/dev/options/definitions
// Array below includes default value, not used, just for documentation
const KNOWN_OPTIONS: &[(&str, OptionType, &str)] = &[
    ("presolve", OptionType::String, "choose"),
    ("solver", OptionType::String, "choose"),
    ("parallel", OptionType::String, "choose"),
    ("run_crossover", OptionType::String, "on"),
    ("time_limit", OptionType::Float, "inf"),
    ("ranging", OptionType::String, "off"),
    ("infinite_cost", OptionType::Float, "1e+20"),
    ("infinite_bound", OptionType::Float, "1e+20"),
    ("small_matrix_value", OptionType::Float, "1e-09"),
    ("large_matrix_value", OptionType::Float, "1e+15"),
    ("kkt_tolerance", OptionType::Float, "1e-07"),
    ("primal_feasibility_tolerance", OptionType::Float, "1e-07"),
    ("dual_feasibility_tolerance", OptionType::Float, "1e-07"),
    ("primal_residual_tolerance", OptionType::Float, "1e-07"),
    ("dual_residual_tolerance", OptionType::Float, "1e-07"),
    ("optimality_tolerance", OptionType::Float, "1e-07"),
    ("objective_bound", OptionType::Float, "inf"),
    ("objective_target", OptionType::Float, "-inf"),
    ("random_seed", OptionType::Int, "0"),
    ("threads", OptionType::Int, "0"),
    ("user_objective_scale", OptionType::Int, "0"),
    ("user_bound_scale", OptionType::Int, "0"),
    ("simplex_strategy", OptionType::Int, "1"),
    ("simplex_scale_strategy", OptionType::Int, "2"),
    ("simplex_dual_edge_weight_strategy", OptionType::Int, "-1"),
    ("simplex_primal_edge_weight_strategy", OptionType::Int, "-1"),
    ("simplex_iteration_limit", OptionType::Int, "2147483647"),
    ("simplex_update_limit", OptionType::Int, "5000"),
    ("simplex_max_concurrency", OptionType::Int, "8"),
    ("output_flag", OptionType::Bool, "true"),
    ("log_to_console", OptionType::Bool, "true"),
    ("log_file", OptionType::String, ""),
    ("write_model_to_file", OptionType::Bool, "false"),
    ("write_presolved_model_to_file", OptionType::Bool, "false"),
    ("write_solution_to_file", OptionType::Bool, "false"),
    ("write_solution_style", OptionType::Int, "0"),
    ("glpsol_cost_row_location", OptionType::Int, "0"),
    ("read_solution_file", OptionType::String, ""),
    ("read_basis_file", OptionType::String, ""),
    ("write_model_file", OptionType::String, ""),
    ("solution_file", OptionType::String, ""),
    ("write_basis_file", OptionType::String, ""),
    ("write_presolved_model_file", OptionType::String, ""),
    ("write_iis_model_file", OptionType::String, ""),
    ("mip_detect_symmetry", OptionType::Bool, "true"),
    ("mip_allow_restart", OptionType::Bool, "true"),
    ("mip_max_nodes", OptionType::Int, "2147483647"),
    ("mip_max_stall_nodes", OptionType::Int, "2147483647"),
    ("mip_max_start_nodes", OptionType::Int, "500"),
    ("mip_improving_solution_save", OptionType::Bool, "false"),
    (
        "mip_improving_solution_report_sparse",
        OptionType::Bool,
        "false",
    ),
    ("mip_improving_solution_file", OptionType::String, ""),
    ("mip_root_presolve_only", OptionType::Bool, "false"),
    ("mip_lifting_for_probing", OptionType::Int, "-1"),
    ("mip_max_leaves", OptionType::Int, "2147483647"),
    ("mip_max_improving_sols", OptionType::Int, "2147483647"),
    ("mip_lp_age_limit", OptionType::Int, "10"),
    ("mip_pool_age_limit", OptionType::Int, "30"),
    ("mip_pool_soft_limit", OptionType::Int, "10000"),
    ("mip_pscost_minreliable", OptionType::Int, "8"),
    (
        "mip_min_cliquetable_entries_for_parallelism",
        OptionType::Int,
        "100000",
    ),
    ("mip_feasibility_tolerance", OptionType::Float, "1e-06"),
    ("mip_heuristic_effort", OptionType::Float, "0.05"),
    (
        "mip_heuristic_run_feasibility_jump",
        OptionType::Bool,
        "true",
    ),
    ("mip_heuristic_run_rins", OptionType::Bool, "true"),
    ("mip_heuristic_run_rens", OptionType::Bool, "true"),
    (
        "mip_heuristic_run_root_reduced_cost",
        OptionType::Bool,
        "true",
    ),
    ("mip_heuristic_run_zi_round", OptionType::Bool, "false"),
    ("mip_heuristic_run_shifting", OptionType::Bool, "false"),
    (
        "mip_allow_cut_separation_at_nodes",
        OptionType::Bool,
        "true",
    ),
    ("mip_rel_gap", OptionType::Float, "0.0001"),
    ("mip_abs_gap", OptionType::Float, "1e-06"),
    ("mip_min_logging_interval", OptionType::Float, "5"),
    ("mip_lp_solver", OptionType::String, "choose"),
    ("mip_ipm_solver", OptionType::String, "choose"),
    ("ipm_optimality_tolerance", OptionType::Float, "1e-08"),
    ("ipm_iteration_limit", OptionType::Int, "2147483647"),
    ("hipo_system", OptionType::String, "choose"),
    ("hipo_parallel_type", OptionType::String, "both"),
    ("hipo_ordering", OptionType::String, "choose"),
    ("hipo_block_size", OptionType::Int, "128"),
    ("pdlp_scaling", OptionType::Bool, "true"),
    ("pdlp_iteration_limit", OptionType::Int, "2147483647"),
    ("pdlp_e_restart_method", OptionType::Int, "1"),
    ("pdlp_optimality_tolerance", OptionType::Float, "1e-07"),
    ("qp_iteration_limit", OptionType::Int, "2147483647"),
    ("qp_nullspace_limit", OptionType::Int, "4000"),
    ("qp_regularization_value", OptionType::Float, "1e-07"),
    ("iis_strategy", OptionType::Int, "0"),
    ("blend_multi_objectives", OptionType::Bool, "true"),
];

fn lookup_option(key: &str) -> Option<OptionType> {
    KNOWN_OPTIONS
        .iter()
        .find(|(name, _, _)| *name == key)
        .map(|(_, t, _)| *t)
}

/// Apply a list of `(key, value)` string pairs as HiGHS solver options.
///
/// Each key is validated against the known option list, and each value is
/// parsed and cast to the appropriate type. All errors are collected and
/// returned together as a single `anyhow::Error`.
///
/// # Example
/// ```ignore
/// let config = vec![
///     ("solver".to_string(), "ipm".to_string()),
///     ("time_limit".to_string(), "30.0".to_string()),
///     ("threads".to_string(), "4".to_string()),
///     ("output_flag".to_string(), "false".to_string()),
/// ];
/// let mut model = ColProblem::default().optimise(Sense::Minimise);
/// apply_options(&mut model, &config).unwrap();
/// ```
pub fn apply_options(model: &mut Model, config: &[(String, String)]) -> Result<()> {
    let errors: Vec<String> = config
        .iter()
        .filter_map(|(key, value)| {
            match lookup_option(key.as_str()) {
                None => Some(format!(
                    "unknown option '{}' (see https://ergo-code.github.io/HiGHS/dev/options/definitions/)",
                    key
                )),
                Some(OptionType::Bool) => {
                    match value.as_str() {
                        "true" | "1" | "on" => { model.set_option(key.as_str(), true); None }
                        "false" | "0" | "off" => { model.set_option(key.as_str(), false); None }
                        _ => Some(format!(
                            "option '{}' expects a boolean (true/false/on/off/1/0), got '{}'",
                            key, value
                        )),
                    }
                }
                Some(OptionType::Int) => {
                    match value.parse::<i32>() {
                        Ok(v) => { model.set_option(key.as_str(), v); None }
                        Err(_) => Some(format!(
                            "option '{}' expects an integer (i32), got '{}'",
                            key, value
                        )),
                    }
                }
                Some(OptionType::Float) => {
                    match value.parse::<f64>() {
                        Ok(v) => { model.set_option(key.as_str(), v); None }
                        Err(_) => Some(format!(
                            "option '{}' expects a float (f64), got '{}'",
                            key, value
                        )),
                    }
                }
                Some(OptionType::String) => {
                    model.set_option(key.as_str(), value.as_str());
                    None
                }
            }
        })
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "invalid HiGHS options:\n{}",
            errors
                .iter()
                .map(|e| format!("  - {}", e))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}
