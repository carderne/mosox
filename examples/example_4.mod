# Example 4: Investment portfolio
set ASSETS;
param returns{a in ASSETS};
param risk{a in ASSETS};
var invest{a in ASSETS} >= 0;
maximize return: sum{a in ASSETS} returns[a] * invest[a];
s.t. budget: sum{a in ASSETS} invest[a] = 1;
s.t. risk_limit: sum{a in ASSETS} risk[a] * invest[a] <= 0.3;

data;
set ASSETS := A1 A2;
param returns := A1 20 A2 50;
param risk := A1 0.1 A2 0.4;

end;
