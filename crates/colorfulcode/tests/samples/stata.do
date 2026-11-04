* Wage regression analysis
version 17
clear all
set more off

// Load and prepare the data
use "data/wages.dta", clear
generate lwage = log(wage)
gen exper2 = exper^2
label variable lwage "Log hourly wage"
drop if missing(educ) | wage <= 0

/* Loop over subgroups */
local groups "male female"
foreach g of local groups {
    display "Group: `g'"
    quietly summarize wage if sex == "`g'", detail
    scalar med_`g' = r(p50)
}

forvalues i = 1/5 {
    replace educ = `i' if educ_cat == `i'
}

global controls "exper exper2 tenure"
regress lwage educ $controls, robust
estimates store base
predict yhat, xb

program define myprog
    syntax varlist [if] [in], [Level(real 95)]
    di as text `"Compound "quoted" string"'
    return scalar n = _N
end

bysort industry: egen mean_wage = mean(wage)
tabulate industry, missing
export delimited using "out/results.csv", replace
