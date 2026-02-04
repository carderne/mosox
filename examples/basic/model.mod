# Example 1: Simple production planning with parameters

# Parameters
param profit1;
param profit2;
param labor_limit;
param material_limit;
param labor1;
param labor2;
param material1;
param material2;

# Variables
var x1 >= 0;
var x2 >= 0;

# Objective
maximize profit: profit1 * x1 + profit2 * x2;

# Constraints
s.t. labor: labor1 * x1 + labor2 * x2 <= labor_limit;
s.t. material: material1 * x1 + material2 * x2 <= material_limit;

solve;

data;

param profit1 := 40;
param profit2 := 30;
param labor_limit := 100;
param material_limit := 80;
param labor1 := 2;
param labor2 := 1;
param material1 := 1;
param material2 := 2;

end;
