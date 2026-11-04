/* Quarterly sales report */
options nodate nonumber;
libname sales '/data/sales';
%let year = 2024;

* Read the raw transactions;
data work.trans;
    infile '/data/raw/trans.csv' dlm=',' firstobs=2;
    input id region $ amount date :yymmdd10.;
    format date date9. amount dollar12.2;
    if amount < 0 then delete;
    quarter = qtr(date);
    where year(date) = &year;
run;

proc sort data=work.trans out=work.sorted nodupkey;
    by region quarter;
run;

proc means data=work.sorted noprint;
    class region quarter;
    var amount;
    output out=work.summary sum=total mean=avg;
run;

%macro report(ds, title=Sales);
    title "&title for &year";
    proc print data=&ds label;
        var region quarter total;
    run;
    %if &sqlobs > 0 %then %put NOTE: printed &ds.;
%mend report;

%report(work.summary, title="Quarterly totals");

proc sql;
    create table work.top as
    select region, sum(amount) as total
    from work.trans
    group by region
    having calculated total > 1e6;
quit;
