# Vars that only appear with zero coefficients should not become columns (as in GLPK)

set I;
param cost{I};
param cap{I};

var x{I} >= 1, <= 10;
var y >= 2;
var z >= 3;

minimize total: sum{i in I} cost[i] * x[i] + 0 * y;

# y cancels out
s.t. limit{i in I}: cap[i] * x[i] + y - y <= 100;
# z only kept for the nonzero term
s.t. other: z + 0 * x['b'] >= 4;

solve;

data;

set I := a b c;
param cost := a 1 b 0 c 2;
param cap := a 3 b 0 c 0;

end;
