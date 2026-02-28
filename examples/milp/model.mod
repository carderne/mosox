var var_flt >= 0;
var var_bin binary;
var var_int integer >= 0;

minimize obj: var_flt + 2*var_bin + 3*var_int;

s.t. c1: var_flt + var_bin + var_int >= 5;
s.t. c2: var_flt >= 1.5;

end;
