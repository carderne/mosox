# Example 3: Transportation problem
set PLANTS;
set WAREHOUSES;
param supply{p in PLANTS};
param demand{w in WAREHOUSES};
param cost{p in PLANTS, w in WAREHOUSES};
var ship{p in PLANTS, w in WAREHOUSES} >= 0;
minimize total_cost: sum{i in PLANTS, j in WAREHOUSES} cost[i,j] * ship[i,j];
s.t. supply_limit{i in PLANTS}: sum{j in WAREHOUSES} ship[i,j] <= supply[i];
s.t. meet_demand{j in WAREHOUSES}: sum{i in PLANTS} ship[i,j] >= demand[j];

data;
set PLANTS := P1 P2;
set WAREHOUSES := W1 W2 W3;
param supply := P1 100  P2 150;
param demand := W1 80  W2 70  W3 50;
param cost :   W1  W2  W3 :=
          P1   10  15  20
          P2   12   8  14;
end;
