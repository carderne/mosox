# Example IR

## Verbose output

If you want full verbose output, you can run this command.
The output is 33,000 lines, so it is not shown here.

```bash
cargo run check --verbose examples/osemosys.mod examples/atlantis.dat
```

## Standard output

This is the output of running the following script in the root of this repository:
```bash
cargo run check examples/osemosys.mod examples/atlantis.dat
```

Output:

```python
minimize cost: <expr>
set DAILYTIMEBRACKET
  data: set DAILYTIMEBRACKET := <2 values>
set DAYTYPE
  data: set DAYTYPE := <1 values>
set EMISSION
  data: set EMISSION := <2 values>
set FUEL
  data: set FUEL := <11 values>
set MODE_OF_OPERATION
  data: set MODE_OF_OPERATION := <2 values>
set REGION
  data: set REGION := <1 values>
set SEASON
  data: set SEASON := <3 values>
set STORAGE
  data: set STORAGE := <1 values>
set TECHNOLOGY
  data: set TECHNOLOGY := <23 values>
set TIMESLICE
  data: set TIMESLICE := <6 values>
set YEAR
  data: set YEAR := <27 values>
param AccumulatedAnnualDemand <domain>
  data: param AccumulatedAnnualDemand default <value>
param AnnualEmissionLimit <domain>
  data: param AnnualEmissionLimit default <value>
param AnnualExogenousEmission <domain>
  data: param AnnualExogenousEmission default <value>
param AvailabilityFactor <domain>
  data: param AvailabilityFactor default <value>
param CapacityFactor <domain>
  data: param CapacityFactor default <value> := <6 table(s)>
param CapacityOfOneTechnologyUnit <domain>
  data: param CapacityOfOneTechnologyUnit default <value>
param CapacityToActivityUnit <domain>
  data: param CapacityToActivityUnit default <value> := <1 table(s)>
param CapitalCost <domain>
  data: param CapitalCost default <value> := <1 table(s)>
param CapitalCostStorage <domain>
  data: param CapitalCostStorage default <value>
param CapitalRecoveryFactor <domain> := <expr>
param Conversionld <domain> binary
  data: param Conversionld default <value> := <1 table(s)>
param Conversionlh <domain> binary
  data: param Conversionlh default <value> := <1 table(s)>
param Conversionls <domain> binary
  data: param Conversionls default <value> := <1 table(s)>
param DaySplit <domain>
  data: param DaySplit default <value>
param DaysInDayType <domain>
  data: param DaysInDayType default <value>
param DepreciationMethod <domain>
  data: param DepreciationMethod default <value>
param DiscountFactor <domain> := <expr>
param DiscountFactorMid <domain> := <expr>
param DiscountFactorMidStorage <domain> := <expr>
param DiscountFactorStorage <domain> := <expr>
param DiscountRate <domain>
  data: param DiscountRate default <value>
param DiscountRateIdv <domain> default <expr>
param DiscountRateStorage <domain>
param EmissionActivityRatio <domain>
  data: param EmissionActivityRatio default <value> := <9 table(s)>
param EmissionsPenalty <domain>
  data: param EmissionsPenalty default <value>
param FixedCost <domain>
  data: param FixedCost default <value> := <1 table(s)>
param InputActivityRatio <domain>
  data: param InputActivityRatio default <value> := <12 table(s)>
param MinStorageCharge <domain>
  data: param MinStorageCharge default <value>
param ModelPeriodEmissionLimit <domain>
  data: param ModelPeriodEmissionLimit default <value>
param ModelPeriodExogenousEmission <domain>
  data: param ModelPeriodExogenousEmission default <value>
param OperationalLife <domain>
  data: param OperationalLife default <value> := <1 table(s)>
param OperationalLifeStorage <domain>
  data: param OperationalLifeStorage default <value>
param OutputActivityRatio <domain>
  data: param OutputActivityRatio default <value> := <22 table(s)>
param PvAnnuity <domain> := <expr>
param REMinProductionTarget <domain>
  data: param REMinProductionTarget default <value> := <1 table(s)>
param RETagFuel <domain> binary
  data: param RETagFuel default <value> := <1 table(s)>
param RETagTechnology <domain> binary
  data: param RETagTechnology default <value> := <1 table(s)>
param ReserveMargin <domain>
  data: param ReserveMargin default <value> := <1 table(s)>
param ReserveMarginTagFuel <domain> binary
  data: param ReserveMarginTagFuel default <value> := <1 table(s)>
param ReserveMarginTagTechnology <domain> <conditions>
  data: param ReserveMarginTagTechnology default <value> := <1 table(s)>
param ResidualCapacity <domain>
  data: param ResidualCapacity default <value> := <1 table(s)>
param ResidualStorageCapacity <domain>
  data: param ResidualStorageCapacity default <value>
param ResultsPath symbolic default <expr>
param SpecifiedAnnualDemand <domain>
  data: param SpecifiedAnnualDemand default <value> := <1 table(s)>
param SpecifiedDemandProfile <domain>
  data: param SpecifiedDemandProfile default <value> := <4 table(s)>
param StorageLevelStart <domain>
  data: param StorageLevelStart default <value>
param StorageMaxChargeRate <domain>
  data: param StorageMaxChargeRate default <value>
param StorageMaxDischargeRate <domain>
  data: param StorageMaxDischargeRate default <value>
param TechnologyFromStorage <domain>
  data: param TechnologyFromStorage default <value> := <1 table(s)>
param TechnologyToStorage <domain>
  data: param TechnologyToStorage default <value> := <1 table(s)>
param TotalAnnualMaxCapacity <domain>
  data: param TotalAnnualMaxCapacity default <value> := <1 table(s)>
param TotalAnnualMaxCapacityInvestment <domain>
  data: param TotalAnnualMaxCapacityInvestment default <value>
param TotalAnnualMinCapacity <domain>
  data: param TotalAnnualMinCapacity default <value>
param TotalAnnualMinCapacityInvestment <domain>
  data: param TotalAnnualMinCapacityInvestment default <value>
param TotalTechnologyAnnualActivityLowerLimit <domain>
  data: param TotalTechnologyAnnualActivityLowerLimit default <value>
param TotalTechnologyAnnualActivityUpperLimit <domain>
  data: param TotalTechnologyAnnualActivityUpperLimit default <value> := <1 table(s)>
param TotalTechnologyModelPeriodActivityLowerLimit <domain>
  data: param TotalTechnologyModelPeriodActivityLowerLimit default <value>
param TotalTechnologyModelPeriodActivityUpperLimit <domain>
  data: param TotalTechnologyModelPeriodActivityUpperLimit default <value>
param TradeRoute <domain> binary
  data: param TradeRoute default <value>
param VariableCost <domain>
  data: param VariableCost default <value> := <16 table(s)>
param YearSplit <domain>
  data: param YearSplit default <value> := <1 table(s)>
var AccumulatedNewCapacity <domain> >= 0
var AccumulatedNewStorageCapacity <domain> >= 0
var AnnualEmissions <domain> >= 0
var AnnualFixedOperatingCost <domain> >= 0
var AnnualTechnologyEmission <domain> >= 0
var AnnualTechnologyEmissionByMode <domain> >= 0
var AnnualTechnologyEmissionPenaltyByEmission <domain> >= 0
var AnnualTechnologyEmissionsPenalty <domain> >= 0
var AnnualVariableOperatingCost <domain> >= 0
var CapitalInvestment <domain> >= 0
var CapitalInvestmentStorage <domain> >= 0
var Demand <domain> >= 0
var DemandNeedingReserveMargin <domain> >= 0
var DiscountedCapitalInvestment <domain> >= 0
var DiscountedCapitalInvestmentStorage <domain> >= 0
var DiscountedOperatingCost <domain> >= 0
var DiscountedSalvageValue <domain> >= 0
var DiscountedSalvageValueStorage <domain> >= 0
var DiscountedTechnologyEmissionsPenalty <domain> >= 0
var ModelPeriodCostByRegion <domain> >= 0
var ModelPeriodEmissions <domain> >= 0
var NetChargeWithinDay <domain>
var NetChargeWithinYear <domain>
var NewCapacity <domain> >= 0
var NewStorageCapacity <domain> >= 0
var NumberOfNewTechnologyUnits <domain> >= 0 integer
var OperatingCost <domain> >= 0
var Production <domain> >= 0
var ProductionAnnual <domain> >= 0
var ProductionByTechnology <domain> >= 0
var ProductionByTechnologyAnnual <domain> >= 0
var RETotalProductionOfTargetFuelAnnual <domain>
var RateOfActivity <domain> >= 0
var RateOfDemand <domain> >= 0
var RateOfProduction <domain> >= 0
var RateOfProductionByTechnology <domain> >= 0
var RateOfProductionByTechnologyByMode <domain> >= 0
var RateOfStorageCharge <domain>
var RateOfStorageDischarge <domain>
var RateOfTotalActivity <domain> >= 0
var RateOfUse <domain> >= 0
var RateOfUseByTechnology <domain> >= 0
var RateOfUseByTechnologyByMode <domain> >= 0
var SalvageValue <domain> >= 0
var SalvageValueStorage <domain> >= 0
var StorageLevelDayTypeFinish <domain> >= 0
var StorageLevelDayTypeStart <domain> >= 0
var StorageLevelSeasonStart <domain> >= 0
var StorageLevelYearFinish <domain> >= 0
var StorageLevelYearStart <domain> >= 0
var StorageLowerLimit <domain> >= 0
var StorageUpperLimit <domain> >= 0
var TotalAnnualTechnologyActivityByMode <domain> >= 0
var TotalCapacityAnnual <domain> >= 0
var TotalCapacityInReserveMargin <domain> >= 0
var TotalDiscountedCost <domain> >= 0
var TotalDiscountedCostByTechnology <domain> >= 0
var TotalDiscountedStorageCost <domain> >= 0
var TotalREProductionAnnual <domain>
var TotalTechnologyAnnualActivity <domain> >= 0
var TotalTechnologyModelPeriodActivity <domain>
var Trade <domain>
var TradeAnnual <domain>
var Use <domain> >= 0
var UseAnnual <domain> >= 0
var UseByTechnology <domain> >= 0
var UseByTechnologyAnnual <domain> >= 0
constraint AAC1_TotalAnnualTechnologyActivity <domain>: <1 constraint expr(s)>
constraint AAC2_TotalAnnualTechnologyActivityUpperLimit <domain>: <1 constraint expr(s)>
constraint AAC3_TotalAnnualTechnologyActivityLowerLimit <domain>: <1 constraint expr(s)>
constraint Acc1_FuelProductionByTechnology <domain>: <1 constraint expr(s)>
constraint Acc2_FuelUseByTechnology <domain>: <1 constraint expr(s)>
constraint Acc3_AverageAnnualRateOfActivity <domain>: <1 constraint expr(s)>
constraint Acc4_ModelPeriodCostByRegion <domain>: <1 constraint expr(s)>
constraint CAa1_TotalNewCapacity <domain>: <1 constraint expr(s)>
constraint CAa2_TotalAnnualCapacity <domain>: <1 constraint expr(s)>
constraint CAa3_TotalActivityOfEachTechnology <domain>: <1 constraint expr(s)>
constraint CAa4_Constraint_Capacity <domain>: <1 constraint expr(s)>
constraint CAa5_TotalNewCapacity <domain>: <1 constraint expr(s)>
constraint CAb1_PlannedMaintenance <domain>: <1 constraint expr(s)>
constraint CC1_UndiscountedCapitalInvestment <domain>: <1 constraint expr(s)>
constraint CC2_DiscountingCapitalInvestment <domain>: <1 constraint expr(s)>
constraint E1_AnnualEmissionProductionByMode <domain>: <1 constraint expr(s)>
constraint E2_AnnualEmissionProduction <domain>: <1 constraint expr(s)>
constraint E3_EmissionsPenaltyByTechAndEmission <domain>: <1 constraint expr(s)>
constraint E4_EmissionsPenaltyByTechnology <domain>: <1 constraint expr(s)>
constraint E5_DiscountedEmissionsPenaltyByTechnology <domain>: <1 constraint expr(s)>
constraint E6_EmissionsAccounting1 <domain>: <1 constraint expr(s)>
constraint E7_EmissionsAccounting2 <domain>: <1 constraint expr(s)>
constraint E8_AnnualEmissionsLimit <domain>: <1 constraint expr(s)>
constraint E9_ModelPeriodEmissionsLimit <domain>: <1 constraint expr(s)>
constraint EBa10_EnergyBalanceEachTS4 <domain>: <1 constraint expr(s)>
constraint EBa11_EnergyBalanceEachTS5 <domain>: <1 constraint expr(s)>
constraint EBa1_RateOfFuelProduction1 <domain>: <1 constraint expr(s)>
constraint EBa2_RateOfFuelProduction2 <domain>: <1 constraint expr(s)>
constraint EBa3_RateOfFuelProduction3 <domain>: <1 constraint expr(s)>
constraint EBa4_RateOfFuelUse1 <domain>: <1 constraint expr(s)>
constraint EBa5_RateOfFuelUse2 <domain>: <1 constraint expr(s)>
constraint EBa6_RateOfFuelUse3 <domain>: <1 constraint expr(s)>
constraint EBa7_EnergyBalanceEachTS1 <domain>: <1 constraint expr(s)>
constraint EBa8_EnergyBalanceEachTS2 <domain>: <1 constraint expr(s)>
constraint EBa9_EnergyBalanceEachTS3 <domain>: <1 constraint expr(s)>
constraint EBb1_EnergyBalanceEachYear1 <domain>: <1 constraint expr(s)>
constraint EBb2_EnergyBalanceEachYear2 <domain>: <1 constraint expr(s)>
constraint EBb3_EnergyBalanceEachYear3 <domain>: <1 constraint expr(s)>
constraint EBb4_EnergyBalanceEachYear4 <domain>: <1 constraint expr(s)>
constraint EQ_SpecifiedDemand <domain>: <1 constraint expr(s)>
constraint NCC1_TotalAnnualMaxNewCapacityConstraint <domain>: <1 constraint expr(s)>
constraint NCC2_TotalAnnualMinNewCapacityConstraint <domain>: <1 constraint expr(s)>
constraint OC1_OperatingCostsVariable <domain>: <1 constraint expr(s)>
constraint OC2_OperatingCostsFixedAnnual <domain>: <1 constraint expr(s)>
constraint OC3_OperatingCostsTotalAnnual <domain>: <1 constraint expr(s)>
constraint OC4_DiscountedOperatingCostsTotalAnnual <domain>: <1 constraint expr(s)>
constraint RE1_FuelProductionByTechnologyAnnual <domain>: <1 constraint expr(s)>
constraint RE2_TechIncluded <domain>: <1 constraint expr(s)>
constraint RE3_FuelIncluded <domain>: <1 constraint expr(s)>
constraint RE4_EnergyConstraint <domain>: <1 constraint expr(s)>
constraint RE5_FuelUseByTechnologyAnnual <domain>: <1 constraint expr(s)>
constraint RM1_ReserveMargin_TechnologiesIncluded_In_Activity_Units <domain>: <1 constraint expr(s)>
constraint RM2_ReserveMargin_FuelsIncluded <domain>: <1 constraint expr(s)>
constraint RM3_ReserveMargin_Constraint <domain>: <1 constraint expr(s)>
constraint S11_and_S12_StorageLevelDayTypeStart <domain>: <1 constraint expr(s)>
constraint S13_and_S14_and_S15_StorageLevelDayTypeFinish <domain>: <1 constraint expr(s)>
constraint S1_RateOfStorageCharge <domain>: <1 constraint expr(s)>
constraint S2_RateOfStorageDischarge <domain>: <1 constraint expr(s)>
constraint S3_NetChargeWithinYear <domain>: <1 constraint expr(s)>
constraint S4_NetChargeWithinDay <domain>: <1 constraint expr(s)>
constraint S5_and_S6_StorageLevelYearStart <domain>: <1 constraint expr(s)>
constraint S7_and_S8_StorageLevelYearFinish <domain>: <1 constraint expr(s)>
constraint S9_and_S10_StorageLevelSeasonStart <domain>: <1 constraint expr(s)>
constraint SC1_LowerLimit_BeginningOfDailyTimeBracketOfFirstInstanceOfDayTypeInFirstWeekConstraint <domain>: <1 constraint expr(s)>
constraint SC1_UpperLimit_BeginningOfDailyTimeBracketOfFirstInstanceOfDayTypeInFirstWeekConstraint <domain>: <1 constraint expr(s)>
constraint SC2_LowerLimit_EndOfDailyTimeBracketOfLastInstanceOfDayTypeInFirstWeekConstraint <domain>: <1 constraint expr(s)>
constraint SC2_UpperLimit_EndOfDailyTimeBracketOfLastInstanceOfDayTypeInFirstWeekConstraint <domain>: <1 constraint expr(s)>
constraint SC3_LowerLimit_EndOfDailyTimeBracketOfLastInstanceOfDayTypeInLastWeekConstraint <domain>: <1 constraint expr(s)>
constraint SC3_UpperLimit_EndOfDailyTimeBracketOfLastInstanceOfDayTypeInLastWeekConstraint <domain>: <1 constraint expr(s)>
constraint SC4_LowerLimit_BeginningOfDailyTimeBracketOfFirstInstanceOfDayTypeInLastWeekConstraint <domain>: <1 constraint expr(s)>
constraint SC4_UpperLimit_BeginningOfDailyTimeBracketOfFirstInstanceOfDayTypeInLastWeekConstraint <domain>: <1 constraint expr(s)>
constraint SC5_MaxChargeConstraint <domain>: <1 constraint expr(s)>
constraint SC6_MaxDischargeConstraint <domain>: <1 constraint expr(s)>
constraint SI10_TotalDiscountedCostByStorage <domain>: <1 constraint expr(s)>
constraint SI1_StorageUpperLimit <domain>: <1 constraint expr(s)>
constraint SI2_StorageLowerLimit <domain>: <1 constraint expr(s)>
constraint SI3_TotalNewStorage <domain>: <1 constraint expr(s)>
constraint SI4_UndiscountedCapitalInvestmentStorage <domain>: <1 constraint expr(s)>
constraint SI5_DiscountingCapitalInvestmentStorage <domain>: <1 constraint expr(s)>
constraint SI6_SalvageValueStorageAtEndOfPeriod1 <domain>: <1 constraint expr(s)>
constraint SI7_SalvageValueStorageAtEndOfPeriod2 <domain>: <1 constraint expr(s)>
constraint SI8_SalvageValueStorageAtEndOfPeriod3 <domain>: <1 constraint expr(s)>
constraint SI9_SalvageValueStorageDiscountedToStartYear <domain>: <1 constraint expr(s)>
constraint SV1_SalvageValueAtEndOfPeriod1 <domain>: <1 constraint expr(s)>
constraint SV2_SalvageValueAtEndOfPeriod2 <domain>: <1 constraint expr(s)>
constraint SV3_SalvageValueAtEndOfPeriod3 <domain>: <1 constraint expr(s)>
constraint SV4_SalvageValueDiscountedToStartYear <domain>: <1 constraint expr(s)>
constraint TAC1_TotalModelHorizonTechnologyActivity <domain>: <1 constraint expr(s)>
constraint TAC2_TotalModelHorizonTechnologyActivityUpperLimit <domain>: <1 constraint expr(s)>
constraint TAC3_TotalModelHorizenTechnologyActivityLowerLimit <domain>: <1 constraint expr(s)>
constraint TCC1_TotalAnnualMaxCapacityConstraint <domain>: <1 constraint expr(s)>
constraint TCC2_TotalAnnualMinCapacityConstraint <domain>: <1 constraint expr(s)>
constraint TDC1_TotalDiscountedCostByTechnology <domain>: <1 constraint expr(s)>
constraint TDC2_TotalDiscountedCost <domain>: <1 constraint expr(s)>
```
