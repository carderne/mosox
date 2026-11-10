NAME model
ROWS
G Demand[101]
G Demand[102]
G Demand[103]
G Demand[201]
G Demand[202]
G Demand[203]
G Later[201,101]
G Later[202,102]
G Later[203,103]
L Ramp[101,103]
L Ramp[102,101]
L Ramp[103,102]
L Ramp[201,203]
L Ramp[202,201]
L Ramp[203,202]
N cost
COLUMNS
up[101] Ramp[101,103] -1
up[101] cost 0.5
up[102] Ramp[102,101] -1
up[102] cost 0.5
up[103] Ramp[103,102] -1
up[103] cost 0.5
up[201] Ramp[201,203] -1
up[201] cost 0.5
up[202] Ramp[202,201] -1
up[202] cost 0.5
up[203] Ramp[203,202] -1
up[203] cost 0.5
x[101] Demand[101] 1
x[101] Later[201,101] -1
x[101] Ramp[101,103] 1
x[101] Ramp[102,101] -1
x[101] cost 1
x[102] Demand[102] 1
x[102] Later[202,102] -1
x[102] Ramp[102,101] 1
x[102] Ramp[103,102] -1
x[102] cost 1
x[103] Demand[103] 1
x[103] Later[203,103] -1
x[103] Ramp[101,103] -1
x[103] Ramp[103,102] 1
x[103] cost 1
x[201] Demand[201] 1
x[201] Later[201,101] 1
x[201] Ramp[201,203] 1
x[201] Ramp[202,201] -1
x[201] cost 1
x[202] Demand[202] 1
x[202] Later[202,102] 1
x[202] Ramp[202,201] 1
x[202] Ramp[203,202] -1
x[202] cost 1
x[203] Demand[203] 1
x[203] Later[203,103] 1
x[203] Ramp[201,203] -1
x[203] Ramp[203,202] 1
x[203] cost 1
RHS
RHS1 Demand[101] 4
RHS1 Demand[102] 5
RHS1 Demand[103] 6
RHS1 Demand[201] 4
RHS1 Demand[202] 5
RHS1 Demand[203] 6
BOUNDS
ENDATA
