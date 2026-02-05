# Example 2: Multi-product with sets
set PRODUCTS;
param profit{p in PRODUCTS};
var production{p in PRODUCTS} >= 0;
maximize total_profit: sum{p in PRODUCTS} profit[p] * production[p];
s.t. capacity: sum{p in PRODUCTS} production[p] <= 1000;
