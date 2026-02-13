# Copied from:
# https://en.wikibooks.org/wiki/GLPK/GMPL_(MathProg)#Official_GLPK_examples

set I;
/* canning plants */

set J;
/* markets */

param a{i in I};
/* capacity of plant i in cases */

param b{j in J};
/* demand at market j in cases */

param d{i in I, j in J};
/* distance in thousands of miles */

param f;
/* freight in dollars per case per thousand miles */

param c{i in I, j in J} := f * d[i,j] / 1000;
/* transport cost in thousands of dollars per case */

var x{i in I, j in J} >= 0;
/* shipment quantities in cases */

minimize cost: sum{i in I, j in J} c[i,j] * x[i,j];
/* total transportation costs in thousands of dollars */

s.t. supply{i in I}: sum{j in J} x[i,j] <= a[i];
/* observe supply limit at plant i */

s.t. demand{j in J}: sum{i in I} x[i,j] >= b[j];
/* satisfy demand at market j */

solve;

# Report / Result Section (Optional)
printf '#################################\n';
printf 'Transportation Problem / LP Model Result\n';
printf '\n';
printf 'Minimum Cost = %.2f\n', cost;
printf '\n';

printf '\n';
printf 'Variables  (i.e. shipment quantities in cases ) \n';

printf 'Shipment quantities in cases\n';
printf 'Canning Plants  Markets   Solution (Cases) \n';
printf{i in I, j in J}:'%14s %10s %11s\n',i,j, x[i,j]; 
printf '\n';

printf 'Constraints\n';
printf '\n';
printf 'Observe supply limit at plant i\n';
printf 'Canning Plants Solution Sign  Required\n';
for {i in I} {
 printf '%14s %10.2f <= %.3f\n', i, sum {j in J} x[i,j], a[i];
   }

printf '\n';
printf 'Satisfy demand at market j\n';
printf 'Market Solution Sign  Required\n';
for {j in J} {
 printf '%5s %10.2f >= %.3f\n', j, sum {i in I} x[i,j], b[j];
   }

data;

set I := Seattle San-Diego;

set J := New-York Chicago Topeka;

param a := Seattle     350
           San-Diego   600;

param b := New-York    325
           Chicago     300
           Topeka      275;

param d :              New-York   Chicago   Topeka :=
           Seattle     2.5        1.7       1.8
           San-Diego   2.5        1.8       1.4  ;

param f := 90;

end;
