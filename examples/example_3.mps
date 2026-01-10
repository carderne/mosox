NAME example_3
ROWS
 N total_cost
 L supply_limit[P1]
 L supply_limit[P2]
 G meet_demand[W1]
 G meet_demand[W2]
 G meet_demand[W3]
COLUMNS
 ship[P1,W1] total_cost 10
 ship[P1,W1] supply_limit[P1] 1
 ship[P1,W1] meet_demand[W1] 1
 ship[P1,W2] total_cost 15
 ship[P1,W2] supply_limit[P1] 1
 ship[P1,W2] meet_demand[W2] 1
 ship[P1,W3] total_cost 20
 ship[P1,W3] supply_limit[P1] 1
 ship[P1,W3] meet_demand[W3] 1
 ship[P2,W1] total_cost 12
 ship[P2,W1] supply_limit[P2] 1
 ship[P2,W1] meet_demand[W1] 1
 ship[P2,W2] total_cost 8
 ship[P2,W2] supply_limit[P2] 1
 ship[P2,W2] meet_demand[W2] 1
 ship[P2,W3] total_cost 14
 ship[P2,W3] supply_limit[P2] 1
 ship[P2,W3] meet_demand[W3] 1
RHS
 RHS1 supply_limit[P1] 100
 RHS1 supply_limit[P2] 150
 RHS1 meet_demand[W1] 80
 RHS1 meet_demand[W2] 70
 RHS1 meet_demand[W3] 50
ENDATA
