NAME model
ROWS
G demand[Chicago]
G demand[New-York]
G demand[Topeka]
L supply[San-Diego]
L supply[Seattle]
N cost
COLUMNS
x[San-Diego,Chicago] cost 0.162
x[San-Diego,Chicago] demand[Chicago] 1
x[San-Diego,Chicago] supply[San-Diego] 1
x[San-Diego,New-York] cost 0.225
x[San-Diego,New-York] demand[New-York] 1
x[San-Diego,New-York] supply[San-Diego] 1
x[San-Diego,Topeka] cost 0.12599999999999997
x[San-Diego,Topeka] demand[Topeka] 1
x[San-Diego,Topeka] supply[San-Diego] 1
x[Seattle,Chicago] cost 0.153
x[Seattle,Chicago] demand[Chicago] 1
x[Seattle,Chicago] supply[Seattle] 1
x[Seattle,New-York] cost 0.225
x[Seattle,New-York] demand[New-York] 1
x[Seattle,New-York] supply[Seattle] 1
x[Seattle,Topeka] cost 0.162
x[Seattle,Topeka] demand[Topeka] 1
x[Seattle,Topeka] supply[Seattle] 1
RHS
RHS1 demand[Chicago] 300
RHS1 demand[New-York] 325
RHS1 demand[Topeka] 275
RHS1 supply[San-Diego] 600
RHS1 supply[Seattle] 350
BOUNDS
ENDATA
