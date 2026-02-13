NAME model
ROWS
G demand[chicago]
G demand[new-york]
G demand[topeka]
L supply[san-diego]
L supply[seattle]
N cost
COLUMNS
shipment[san-diego,chicago] cost 0.162
shipment[san-diego,chicago] demand[chicago] 1
shipment[san-diego,chicago] supply[san-diego] 1
shipment[san-diego,new-york] cost 0.225
shipment[san-diego,new-york] demand[new-york] 1
shipment[san-diego,new-york] supply[san-diego] 1
shipment[san-diego,topeka] cost 0.12599999999999997
shipment[san-diego,topeka] demand[topeka] 1
shipment[san-diego,topeka] supply[san-diego] 1
shipment[seattle,chicago] cost 0.153
shipment[seattle,chicago] demand[chicago] 1
shipment[seattle,chicago] supply[seattle] 1
shipment[seattle,new-york] cost 0.225
shipment[seattle,new-york] demand[new-york] 1
shipment[seattle,new-york] supply[seattle] 1
shipment[seattle,topeka] cost 0.162
shipment[seattle,topeka] demand[topeka] 1
shipment[seattle,topeka] supply[seattle] 1
RHS
RHS1 demand[chicago] 300
RHS1 demand[new-york] 325
RHS1 demand[topeka] 275
RHS1 supply[san-diego] 600
RHS1 supply[seattle] 350
BOUNDS
ENDATA
