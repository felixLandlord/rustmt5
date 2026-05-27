//+------------------------------------------------------------------+
//| Strategy: EMA Crossover                                          |
//| File:     strategy.mq5                                           |
//|                                                                  |
//| A simple trend-following Expert Advisor that trades crossovers   |
//| between a fast EMA (Exponential Moving Average) and a slow EMA.  |
//|                                                                  |
//| How it works:                                                    |
//|   - When the fast EMA (period 9) crosses above the slow EMA     |
//|     (period 21), a BUY signal is generated.                      |
//|   - When the fast EMA crosses below the slow EMA, a SELL signal  |
//|     is generated.                                                 |
//|   - Only one position is open at a time. A new signal in the     |
//|     opposite direction closes the current position and opens     |
//|     a new one.                                                   |
//|   - A fixed stop loss and take profit (in points) protect each   |
//|     trade.                                                       |
//+------------------------------------------------------------------+
#property copyright "rustmt5 example"
#property link      "https://github.com/felixLandlord/rustmt5"
#property version   "1.00"
#property strict

input int    FastPeriod  = 9;       // Fast EMA period
input int    SlowPeriod  = 21;      // Slow EMA period
input double LotSize     = 0.1;     // Trade lot size
input int    StopLoss    = 300;     // Stop loss in points
input int    TakeProfit  = 600;     // Take profit in points
input int    MagicNumber = 100001;  // EA magic number

int handleFast;
int handleSlow;

//+------------------------------------------------------------------+
//| Expert initialization                                            |
//+------------------------------------------------------------------+
int OnInit()
{
   handleFast = iMA(_Symbol, PERIOD_CURRENT, FastPeriod, 0, MODE_EMA, PRICE_CLOSE);
   handleSlow = iMA(_Symbol, PERIOD_CURRENT, SlowPeriod, 0, MODE_EMA, PRICE_CLOSE);

   if(handleFast == INVALID_HANDLE || handleSlow == INVALID_HANDLE)
   {
      Print("Failed to create indicator handles");
      return INIT_FAILED;
   }

   return INIT_SUCCEEDED;
}

//+------------------------------------------------------------------+
//| Expert deinitialization                                          |
//+------------------------------------------------------------------+
void OnDeinit(const int reason)
{
   IndicatorRelease(handleFast);
   IndicatorRelease(handleSlow);
}

//+------------------------------------------------------------------+
//| Expert tick function                                             |
//+------------------------------------------------------------------+
void OnTick()
{
   double fast[], slow[];
   ArraySetAsSeries(fast, true);
   ArraySetAsSeries(slow, true);

   if(CopyBuffer(handleFast, 0, 0, 3, fast) < 3) return;
   if(CopyBuffer(handleSlow, 0, 0, 3, slow) < 3) return;

   // Detect crossover using the two most recent completed bars (index 1 and 2)
   bool crossUp   = fast[2] <= slow[2] && fast[1] > slow[1];
   bool crossDown = fast[2] >= slow[2] && fast[1] < slow[1];

   if(crossUp)
   {
      ClosePositions(POSITION_TYPE_SELL);
      if(!HasPosition(POSITION_TYPE_BUY))
         OpenTrade(ORDER_TYPE_BUY);
   }
   else if(crossDown)
   {
      ClosePositions(POSITION_TYPE_BUY);
      if(!HasPosition(POSITION_TYPE_SELL))
         OpenTrade(ORDER_TYPE_SELL);
   }
}

//+------------------------------------------------------------------+
//| Open a trade with SL and TP                                      |
//+------------------------------------------------------------------+
void OpenTrade(ENUM_ORDER_TYPE type)
{
   double price = (type == ORDER_TYPE_BUY) ? SymbolInfoDouble(_Symbol, SYMBOL_ASK)
                                           : SymbolInfoDouble(_Symbol, SYMBOL_BID);
   double point = SymbolInfoDouble(_Symbol, SYMBOL_POINT);
   double sl, tp;

   if(type == ORDER_TYPE_BUY)
   {
      sl = price - StopLoss  * point;
      tp = price + TakeProfit * point;
   }
   else
   {
      sl = price + StopLoss  * point;
      tp = price - TakeProfit * point;
   }

   MqlTradeRequest request = {};
   MqlTradeResult  result  = {};

   request.action   = TRADE_ACTION_DEAL;
   request.symbol   = _Symbol;
   request.volume   = LotSize;
   request.type     = type;
   request.price    = price;
   request.sl       = sl;
   request.tp       = tp;
   request.magic    = MagicNumber;
   request.deviation = 10;
   request.comment  = "EMA Crossover";

   if(!OrderSend(request, result))
      Print("OrderSend failed: ", GetLastError());
}

//+------------------------------------------------------------------+
//| Check if a position of given type exists                         |
//+------------------------------------------------------------------+
bool HasPosition(ENUM_POSITION_TYPE type)
{
   for(int i = PositionsTotal() - 1; i >= 0; i--)
   {
      if(PositionGetSymbol(i) == _Symbol &&
         PositionGetInteger(POSITION_MAGIC) == MagicNumber &&
         PositionGetInteger(POSITION_TYPE) == type)
         return true;
   }
   return false;
}

//+------------------------------------------------------------------+
//| Close all positions of given type                                |
//+------------------------------------------------------------------+
void ClosePositions(ENUM_POSITION_TYPE type)
{
   for(int i = PositionsTotal() - 1; i >= 0; i--)
   {
      if(PositionGetSymbol(i) != _Symbol) continue;
      if(PositionGetInteger(POSITION_MAGIC) != MagicNumber) continue;
      if(PositionGetInteger(POSITION_TYPE)  != type) continue;

      ulong ticket = PositionGetInteger(POSITION_TICKET);

      MqlTradeRequest request = {};
      MqlTradeResult  result  = {};

      request.action   = TRADE_ACTION_DEAL;
      request.symbol   = _Symbol;
      request.position = ticket;
      request.volume   = PositionGetDouble(POSITION_VOLUME);
      request.type     = (type == POSITION_TYPE_BUY) ? ORDER_TYPE_SELL : ORDER_TYPE_BUY;
      request.price    = (type == POSITION_TYPE_BUY) ? SymbolInfoDouble(_Symbol, SYMBOL_BID)
                                                     : SymbolInfoDouble(_Symbol, SYMBOL_ASK);
      request.deviation = 10;

      if(!OrderSend(request, result))
         Print("Close failed: ", GetLastError());
   }
}
