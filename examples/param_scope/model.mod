# Regression model for three GMPL features that glpsol supports:
#  1. a computed param read at two different indices in one condition
#     (its expression must use its own dummy, not the caller's);
#  2. iterated min/max over a set: min{h in HOUR} h;
#  3. a comment inside an expression.
# PREV links each timeslice to the previous hour of the same season
# (the first hour wraps around to the last).
set SEASON;
set HOUR;
set TIMESLICE;
param InSeason{l in TIMESLICE, s in SEASON};
param AtHour{l in TIMESLICE, h in HOUR};
param TsSeason{l in TIMESLICE} := sum{s in SEASON} s * InSeason[l, s];
param TsHour{l in TIMESLICE} := sum{h in HOUR} h * AtHour[l, h];
param FirstHour := min{h in HOUR} h;
param LastHour := max{h in HOUR} h;
set PREV := {l in TIMESLICE, lp in TIMESLICE:
             TsSeason[lp] = TsSeason[l]
             && TsHour[lp] = (if TsHour[l] = FirstHour then LastHour else TsHour[l] - 1)};
var x{TIMESLICE} >= 0;
var up{TIMESLICE} >= 0;
minimize cost:
    sum{l in TIMESLICE} x[l]
    # upward changes are penalised
    + sum{l in TIMESLICE} 0.5 * up[l];
s.t. Ramp{(l, lp) in PREV}: x[l] - x[lp] <= up[l];
s.t. Later{l in TIMESLICE, lp in TIMESLICE: TsSeason[lp] < TsSeason[l] && TsHour[lp] = TsHour[l]}:
    x[l] >= x[lp];
s.t. Demand{l in TIMESLICE}: x[l] >= TsHour[l] + FirstHour * LastHour;
solve;
end;
