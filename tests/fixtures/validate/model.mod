set I;
set J;
set P dimen 2 within I cross J;
set S{i in I} within J;
set T within {i in I, j in J};

param cap;
param n{I} integer, >= 0, <= cap;
param b{I} binary;
param lim{i in I} <= n[i];
param name{I} symbolic in J;
param r{i in I, j in S[i]};

var x{I} >= 0;

minimize obj: sum{i in I} x[i];
s.t. c{i in I}: x[i] >= n[i];

check{i in I}: lim[i] <= 1;
