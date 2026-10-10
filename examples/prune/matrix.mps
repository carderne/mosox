NAME model
ROWS
G other
L limit[a]
L limit[b]
L limit[c]
N total
COLUMNS
x[a] limit[a] 3
x[a] total 1
x[c] total 2
z other 1
RHS
RHS1 limit[a] 100
RHS1 limit[b] 100
RHS1 limit[c] 100
RHS1 other 4
BOUNDS
LO BND1 x[a] 1
LO BND1 x[c] 1
LO BND1 z 3
UP BND1 x[a] 10
UP BND1 x[c] 10
ENDATA
