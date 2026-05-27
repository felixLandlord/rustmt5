//+------------------------------------------------------------------+
//| Strategy: EMA Crossover (BROKEN)                                 |
//| File:     strategy_error.mq5                                     |
//|                                                                  |
//| This file intentionally contains a compilation error.            |
//| Purpose: demonstrate rustmt5 compile failure behavior.           |
//|                                                                  |
//| The bug: OnStart uses 'Prnt' (typo) instead of 'Print'.          |
//+------------------------------------------------------------------+
#property copyright "rustmt5 example"
#property link      "https://github.com/felixLandlord/rustmt5"
#property version   "1.00"
#property strict

//+------------------------------------------------------------------+
//| Expert initialization                                            |
//+------------------------------------------------------------------+
int OnInit()
{
   return INIT_SUCCEEDED;
}

//+------------------------------------------------------------------+
//| Expert deinitialization                                          |
//+------------------------------------------------------------------+
void OnDeinit(const int reason)
{
}

//+------------------------------------------------------------------+
//| Expert tick function                                             |
//+------------------------------------------------------------------+
void OnTick()
{
   // BUG: 'Prnt' is not declared — should be 'Print'
   Prnt("Hello from broken EA");
}
