//+------------------------------------------------------------------+
//| Moving average crossover expert advisor                          |
//+------------------------------------------------------------------+
#property copyright "Example"
#property version   "1.00"
#include <Trade\Trade.mqh>

input int    FastPeriod = 12;
input int    SlowPeriod = 26;
input double LotSize    = 0.1;

CTrade trade;
int fastHandle, slowHandle;

class CSignal {
public:
    bool IsCross(const double &fast[], const double &slow[]) const {
        return fast[1] < slow[1] && fast[0] > slow[0];
    }
};

int OnInit() {
    fastHandle = iMA(_Symbol, PERIOD_CURRENT, FastPeriod, 0, MODE_EMA, PRICE_CLOSE);
    slowHandle = iMA(_Symbol, PERIOD_CURRENT, SlowPeriod, 0, MODE_EMA, PRICE_CLOSE);
    if (fastHandle == INVALID_HANDLE) {
        Print("Failed to create indicator: ", GetLastError());
        return INIT_FAILED;
    }
    return INIT_SUCCEEDED;
}

void OnTick() {
    double fast[], slow[];
    ArraySetAsSeries(fast, true);
    CopyBuffer(fastHandle, 0, 0, 2, fast);
    CopyBuffer(slowHandle, 0, 0, 2, slow);
    CSignal signal;
    double ask = NormalizeDouble(SymbolInfoDouble(_Symbol, SYMBOL_ASK), _Digits);
    if (signal.IsCross(fast, slow) && !PositionSelect(_Symbol)) {
        trade.Buy(LotSize, _Symbol, ask, 0, 0, "MA cross\n");
    }
    datetime now = TimeCurrent();
    color c = clrRed;
}
