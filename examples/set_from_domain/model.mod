# This model isn't solvable
# Just here to test some index/domain behaviour

set YEAR;
set FUEL;
set BLOB{FUEL} dimen 2;

set GEN := setof {y in YEAR, f in FUEL, (b1,b2) in BLOB[f]} (y,f,b1,b2);
set BOB := {y in YEAR, f in FUEL, (b1,b2) in BLOB[f]};

var Foo{(y,f,b1,b2) in GEN};
var Bar{(y,f,b1,b2) in BOB};

minimize cost: sum {(y,f,b1,b2) in GEN} (Foo[y,f,b1,b2] + Bar[y,f,b1,b2]);

data;
set YEAR := 2020, 2021;
set FUEL := A, B, C;
set BLOB[A] := (1, 2);
set BLOB[B] := (5, 6);
set BLOB[C] := (8, 9);

end;
