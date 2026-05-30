# rustmt5 Implementation Spec — Metrics & Scoring

## Overview

This spec defines the complete implementation for:
1. **Metrics Extraction**: Parse MT5 HTML reports → extract metrics → validate → save to JSON
2. **Scoring**: Load score config + metrics JSON → validate → calculate score → report results

---

## Part 1: Metrics Extraction

### 1.1 CLI Command

```bash
rustmt5 metrics <report.htm> [-o <output>] [--append <file>]
```

**Arguments:**
- `<report.htm>` (required) — Path to MT5 HTML report file

**Options:**
- `-o, --output <path>` — Custom output path (default: `output/metrics/{report_name}.json`)
- `--append <file>` — Append to existing metrics JSON (increment ID, preserve existing reports)

---

### 1.2 Execution Flow

```
1. Parse HTML report
2. Extract all metrics (73 total)
3. Validate extraction:
   - All required metric keys present
   - All values match expected types
   - No NaN/Infinity values
4. Build JSON structure
5. Handle --append flag
6. Create output directory if needed
7. Write JSON file
8. Report success/failure
```

---

### 1.3 Success Output

```
✓ Extracted metrics from strategy_report.htm
  Metrics: 73 / 73 present and valid
  Report ID: 1
  Saved to: output/metrics/strategy_report.json
```

---

### 1.4 Failure Output (Examples)

**Missing metric:**
```
✗ Invalid metrics in report: strategy_report.htm
  Results section:
    - Missing key: "profit_factor"
    - Missing key: "sharpe_ratio"
```

**Type mismatch:**
```
✗ Invalid metrics in report: strategy_report.htm
  Results section:
    - Type error: "balance" (line 42)
      Expected: float
      Got: string "9,164.24"
    - Type error: "total_trades" (line 15)
      Expected: integer
      Got: string "33,999"
```

**Invalid numeric value:**
```
✗ Invalid metrics in report: strategy_report.htm
  Results section:
    - Invalid value: "sharpe_ratio" = "N/A"
      Expected: numeric (float)
    - Invalid value: "z_score_%" = "NaN"
      Cannot process NaN values
```

**Append flag issues:**
```
✗ Cannot append to metrics file
  File not found: ./my_metrics.json
  
  Hint: Omit --append flag to create a new file, or specify a valid existing file.
```

---

### 1.5 Metrics JSON Schema

**Output file structure:**

```json
{
  "report(s)": [
    {
      "id": 1,
      "settings": {
        "expert": "rustmt5_ea/MyEA",
        "symbol": "USA100",
        "period": {
          "timeframe": "M5",
          "from_date": "2018.08.01",
          "to_date": "2026.05.01"
        },
        "inputs": {},
        "company": "MetaQuotes",
        "currency": "USD",
        "initial_deposit": 10000,
        "leverage": "1:500"
      },
      "results": {
        "history_quality_%": 98.0,
        "bars": 550057,
        "total_net_profit": -835.76,
        "gross_profit": 5467.68,
        "gross_loss": -6303.44,
        "profit_factor": 0.87,
        "recovery_factor": -0.99,
        "AHPR": 1.0,
        "AHPR_%": -0.0,
        "GHPR": 1.0,
        "GHPR_%": -0.0,
        "total_trades": 33999,
        "total_deals": 67998,
        "ticks": 10078513,
        "balance_drawdown_absolute": 839.68,
        "balance_drawdown_maximal": 839.68,
        "balance_drawdown_maximal_%": 8.40,
        "balance_drawdown_relative_%": 8.40,
        "balance_drawdown_relative": 839.68,
        "expected_payoff": -0.02,
        "sharpe_ratio": -5.0,
        "lr_correlation": -1.0,
        "lr_standard_error": 22.84,
        "short_trades": 17255,
        "short_trades_won_%": 28.85,
        "profit_trades": 10100,
        "profit_trades_% (of total)": 29.71,
        "largest_profit_trade": 0.67,
        "average_profit_trade": 0.54,
        "maximum_consecutive_wins": 8,
        "amount_from_maximum_consecutive_wins": 4.65,
        "maximal_consecutive_profit": 4.65,
        "maximal_consecutive_profit_count": 8,
        "average_consecutive_wins": 1,
        "symbols": 1,
        "equity_drawdown_absolute": 840.33,
        "equity_drawdown_maximal": 840.39,
        "equity_drawdown_maximal_%": 8.40,
        "equity_drawdown_relative_%": 8.40,
        "equity_drawdown_relative": 840.39,
        "margin_level_%": 1272320.83,
        "z_score": 14.53,
        "z_score_%": 99.74,
        "ontester_result": 0,
        "long_trades": 16744,
        "long_trades_won_%": 30.59,
        "loss_trades": 23899,
        "loss_trades_% (of total)": 70.29,
        "largest_loss_trade": -0.9,
        "average_loss_trade": -0.26,
        "maximum_consecutive_losses": 27,
        "amount_from_maximum_consecutive_losses": -4.84,
        "maximal_consecutive_loss": -6.68,
        "maximal_consecutive_loss_count": 26,
        "average_consecutive_losses": 3,
        "correlation (Profits, MFE)": 0.8,
        "correlation (Profits, MAE)": 0.54,
        "correlation (MFE, MAE)": 0.3791,
        "minimal_positon_holding_time": "00:00:01",
        "maximal_positon_holding_time": "30:35:00",
        "average_positon_holding_time": "00:31:54",
        "commission": 0.0,
        "swap": -57.09,
        "profit": -778.67,
        "balance": 9164.24
      }
    }
  ]
}
```

**Validation rules for extraction:**
- All `results` keys must be present
- Type mapping:
  - `history_quality_%`, `bars`, `total_deals`, `ticks`, `total_trades`, `short_trades`, `profit_trades`, `long_trades`, `loss_trades`, `maximum_consecutive_wins`, `maximum_consecutive_losses`, `symbols`, `maximal_consecutive_profit_count`, `maximal_consecutive_loss_count`, `average_consecutive_wins`, `average_consecutive_losses` → **integer**
  - All percentage fields (ending with `_%`) → **float**
  - All remaining numeric fields → **float**
  - Time fields (`*_holding_time`) → **string** (format: `HH:MM:SS`)
- No NaN, Infinity, or null values allowed in `results`
- All required keys in `settings` must be present

---

### 1.6 Append Behavior

When using `--append <file>`:

```json
{
  "report(s)": [
    { "id": 1, ... },
    { "id": 2, ... },
    { "id": 3, ... }
  ]
}
```

**Rules:**
- Calculate next ID: `max_id + 1`
- Preserve all existing reports
- Append new report at end
- Maintain chronological order

**Example:**
```bash
rustmt5 metrics report1.htm -o metrics.json
# Creates: metrics.json with 1 report (id=1)

rustmt5 metrics report2.htm --append metrics.json
# Updates: metrics.json with 2 reports (id=1, id=2)

rustmt5 metrics report3.htm --append metrics.json
# Updates: metrics.json with 3 reports (id=1, id=2, id=3)
```

---

## Part 2: Scoring

### 2.1 CLI Command

```bash
rustmt5 score <config.toml> <metrics.json>
```

**Arguments:**
- `<config.toml>` (required) — Path to score configuration file
- `<metrics.json>` (required) — Path to extracted metrics JSON file

---

### 2.2 Execution Flow

```
1. Load and parse config.toml
2. Validate config structure:
   - scoring.method is valid (weighted_sum, weighted_average, geometric_mean, harmonic_mean, exponential_weighted)
   - All metric names exist in allowed list
   - All weights are valid numbers (>= 0)
   - Weight sum = 100 (if applicable)
   - All required fields present
3. Load and parse metrics.json
4. Validate metrics JSON:
   - All required metric keys present
   - All values are correct types
   - No NaN/Infinity values
5. For each report in metrics.json:
   - Normalize each metric to 0–100 scale
   - Apply scoring method
   - Calculate final score
6. Output results for all reports
7. Report success/failure
```

---

### 2.3 Success Output (Single Report)

```
✓ Score calculated successfully
  Config: score.toml (method: weighted_average)
  Metrics: metrics.json (report ID: 1)
  
  Results:
    Report ID: 1
    Score: 65.3 / 100
    Status: PASS (threshold: 60.0)
    
  Breakdown (weighted_average):
    profit_factor (30%):          75.0 → 22.5
    sharpe_ratio (25%):           60.0 → 15.0
    balance_drawdown_maximal_% (25%): 50.0 → 12.5
    recovery_factor (20%):        40.0 → 8.0
    z_score_% (0%):               99.7 → 0.0
                                  ─────────────
    Final Score:                  58.0 / 100
    
  Status: PASS (≥ 60.0)
```

---

### 2.4 Success Output (Multiple Reports)

```
✓ Score calculated for 3 reports
  Config: score.toml (method: weighted_average)
  Metrics: metrics.json
  
  Summary:
    Report 1: 65.3 / 100 (PASS)
    Report 2: 42.1 / 100 (FAIL)
    Report 3: 78.9 / 100 (PASS)
    
  Pass Rate: 66.7% (2/3)
```

---

### 2.5 Failure Output (Examples)

**Config validation error:**
```
✗ Invalid score configuration: score.toml
  
  Config errors:
    - Unknown scoring method: "weighted_sum_custom"
      Allowed: weighted_sum, weighted_average, geometric_mean, harmonic_mean, exponential_weighted
    
    - Unknown metric: "profit_factor_adjusted"
      Allowed: profit_factor, sharpe_ratio, recovery_factor, expected_payoff, ...
    
    - Invalid weight: "balance_drawdown_maximal_%"
      Weight must be >= 0, got: -5.0
    
    - Weight sum: 95.0
      Expected: 100.0 (or 0 if weights are relative)
    
    - Missing required field: "pass_threshold"
      (only if method requires it)
```

**Metrics validation error:**
```
✗ Invalid metrics file: metrics.json
  
  Metrics errors:
    - Report 1: Missing key "profit_factor"
    - Report 2: Type error "sharpe_ratio" (expected float, got string "N/A")
    - Report 3: Invalid value "balance_drawdown_maximal_%" = NaN
```

**Both errors:**
```
✗ Cannot calculate score
  
  Config errors (score.toml):
    - Weight sum: 95.0 (expected 100.0)
    - Unknown metric: "profit_factor_x"
  
  Metrics errors (metrics.json):
    - Report 1: Missing key "balance_drawdown_maximal_%"
    - Report 1: Invalid value "sharpe_ratio" = "N/A"
```

---

### 2.6 Score Config Format (score.toml)

#### 2.6.1 Basic Example (Weighted Average)

```toml
# score.toml — Simple balanced scoring

[scoring]
method = "weighted_average"
pass_threshold = 60.0

[[metrics]]
name = "profit_factor"
weight = 30.0
direction = "higher_is_better"

[[metrics]]
name = "sharpe_ratio"
weight = 25.0
direction = "higher_is_better"

[[metrics]]
name = "balance_drawdown_maximal_%"
weight = 25.0
direction = "lower_is_better"

[[metrics]]
name = "recovery_factor"
weight = 20.0
direction = "higher_is_better"
```

**Explanation:**
- `method`: Algorithm used for calculation (weighted_average)
- `pass_threshold`: Score must be ≥ 60.0 to pass (0–100)
- Each `[[metrics]]` entry defines a metric to score:
  - `name`: Exact metric key from metrics.json
  - `weight`: Relative importance (0–100, should sum to 100 for weighted_average)
  - `direction`: Whether higher/lower values are better

---

#### 2.6.2 Advanced Example (with Normalization)

```toml
# score.toml — Conservative risk-first model with min/cap values

[scoring]
method = "weighted_average"
pass_threshold = 65.0

[[metrics]]
name = "profit_factor"
weight = 35.0
direction = "higher_is_better"
min_value = 0.8      # Below 0.8, score is 0 (losing strategy disqualified)
cap_value = 4.0      # Above 4.0, treated as 4.0 (diminishing returns)

[[metrics]]
name = "balance_drawdown_maximal_%"
weight = 30.0
direction = "lower_is_better"
cap_value = 40.0     # Drawdown above 40% is treated as worst case

[[metrics]]
name = "sharpe_ratio"
weight = 20.0
direction = "higher_is_better"
min_value = 0.0      # Negative sharpe scores 0

[[metrics]]
name = "z_score_%"
weight = 15.0
direction = "higher_is_better"
min_value = 50.0     # Confidence below 50% scores 0
```

**New fields:**
- `min_value` (optional): Values below this score 0. Disqualifies poor performance.
- `cap_value` (optional): Values above this are capped. Prevents outliers from dominating.

---

#### 2.6.3 Geometric Mean Example

```toml
# score.toml — Balanced, penalizes weak links

[scoring]
method = "geometric_mean"
pass_threshold = 60.0

[[metrics]]
name = "profit_factor"
weight = 30.0
direction = "higher_is_better"
min_value = 1.0      # Strategy must be profitable (PF >= 1.0)

[[metrics]]
name = "sharpe_ratio"
weight = 30.0
direction = "higher_is_better"
min_value = 0.5

[[metrics]]
name = "balance_drawdown_maximal_%"
weight = 25.0
direction = "lower_is_better"
cap_value = 35.0

[[metrics]]
name = "recovery_factor"
weight = 15.0
direction = "higher_is_better"
```

---

#### 2.6.4 Harmonic Mean Example (Ultra-Conservative)

```toml
# score.toml — Harmonic mean: all metrics must be solid

[scoring]
method = "harmonic_mean"
pass_threshold = 70.0

[[metrics]]
name = "profit_factor"
weight = 30.0
direction = "higher_is_better"
min_value = 1.2      # Must be strong (at least 1.2)
cap_value = 3.0

[[metrics]]
name = "sharpe_ratio"
weight = 25.0
direction = "higher_is_better"
min_value = 1.0      # Must be positive and meaningful

[[metrics]]
name = "z_score_%"
weight = 25.0
direction = "higher_is_better"
min_value = 90.0     # Need 90%+ confidence

[[metrics]]
name = "balance_drawdown_maximal_%"
weight = 20.0
direction = "lower_is_better"
cap_value = 25.0     # Max 25% drawdown acceptable
```

---

#### 2.6.5 Exponential Weighted Example

```toml
# score.toml — Exponential: emphasizes best performers

[scoring]
method = "exponential_weighted"
decay = 1.5          # Controls exponential curve (0 < decay < 2)
pass_threshold = 60.0

[[metrics]]
name = "profit_factor"
weight = 40.0
direction = "higher_is_better"

[[metrics]]
name = "sharpe_ratio"
weight = 30.0
direction = "higher_is_better"

[[metrics]]
name = "recovery_factor"
weight = 20.0
direction = "higher_is_better"

[[metrics]]
name = "balance_drawdown_maximal_%"
weight = 10.0
direction = "lower_is_better"
```

**New field for exponential:**
- `decay` (0 < decay < 2): Controls how aggressive the exponential function is
  - decay = 0.5: Slight penalty for weak metrics
  - decay = 1.0: Moderate penalty
  - decay = 1.5: Aggressive penalty for weak metrics
  - decay = 1.9: Extreme penalty (one weak metric ruins score)

---

### 2.7 Allowed Metrics (for validation)

These are the only metric names allowed in `[[metrics]]` entries. Config validation must check against this list.

```
profit_factor
recovery_factor
AHPR
AHPR_%
GHPR
GHPR_%
total_trades
total_deals
ticks
balance_drawdown_absolute
balance_drawdown_maximal
balance_drawdown_maximal_%
balance_drawdown_relative_%
balance_drawdown_relative
expected_payoff
sharpe_ratio
lr_correlation
lr_standard_error
short_trades
short_trades_won_%
profit_trades
profit_trades_% (of total)
largest_profit_trade
average_profit_trade
maximum_consecutive_wins
amount_from_maximum_consecutive_wins
maximal_consecutive_profit
maximal_consecutive_profit_count
average_consecutive_wins
symbols
equity_drawdown_absolute
equity_drawdown_maximal
equity_drawdown_maximal_%
equity_drawdown_relative_%
equity_drawdown_relative
margin_level_%
z_score
z_score_%
ontester_result
long_trades
long_trades_won_%
loss_trades
loss_trades_% (of total)
largest_loss_trade
average_loss_trade
maximum_consecutive_losses
amount_from_maximum_consecutive_losses
maximal_consecutive_loss
maximal_consecutive_loss_count
average_consecutive_losses
correlation (Profits, MFE)
correlation (Profits, MAE)
correlation (MFE, MAE)
history_quality_%
bars
total_net_profit
gross_profit
gross_loss
commission
swap
profit
balance
```

---

### 2.8 Scoring Methods (Formulas & Examples)

#### 2.8.1 Weighted Sum

**Formula:**
```
normalized_value = (metric_value - min) / (max - min) × 100

score = (Σ(normalized_value × weight)) / (Σ(weight)) × 100
```

**Why:**
- Simple, transparent, linear
- "Doubling profit factor doubles its contribution"
- Industry standard
- Easiest to debug and explain

**Example with 3 metrics (weights: 30, 40, 30, total 100):**
```
Raw values:
  profit_factor = 1.5
  sharpe_ratio = 1.2
  balance_drawdown_maximal_% = 15.0

Assume normalization bounds (industry standard):
  profit_factor: [0, 3] → 1.5 normalized to (1.5 / 3) × 100 = 50.0
  sharpe_ratio: [0, 3] → 1.2 normalized to (1.2 / 3) × 100 = 40.0
  drawdown: [0, 100] → 15.0 normalized to ((100 - 15) / 100) × 100 = 85.0
    (lower is better, so inverted)

Weighted sum:
  (50.0 × 30 + 40.0 × 40 + 85.0 × 30) / 100
  = (1500 + 1600 + 2550) / 100
  = 5650 / 100
  = 56.5 / 100
```

---

#### 2.8.2 Weighted Average

**Formula:**
```
score = Σ(normalized_value × weight) / Σ(weight)
```

**Why:**
- Cleaner than weighted sum
- "Average quality of this strategy"
- Doesn't require weights to sum to 100 (more flexible)
- Still fully transparent

**Same example:**
```
Weighted average:
  (50.0 × 30 + 40.0 × 40 + 85.0 × 30) / (30 + 40 + 30)
  = 5650 / 100
  = 56.5
  
(Same answer, but cleaner formula if weights don't sum to 100)
```

---

#### 2.8.3 Geometric Mean

**Formula:**
```
score = (∏(normalized_value ^ (weight / Σ(weight)))) ^ 100
      = 100 × (metric1^w1 × metric2^w2 × ... × metricN^wN) ^ (1 / Σ(weight))

where ∏ is product (multiply all), w1, w2, etc. are weights
```

**Why:**
- Penalizes weak links more than weighted sum
- "No single metric can hide bad performance in another"
- Used in quant finance (Sortino, Calmar)
- Resistant to gaming

**Example (same 3 metrics, weights normalized to proportions):**
```
Proportions: 30/100 = 0.3, 40/100 = 0.4, 30/100 = 0.3

Normalized values:
  metric1 = 0.50  (50.0 / 100)
  metric2 = 0.40  (40.0 / 100)
  metric3 = 0.85  (85.0 / 100)

Geometric mean:
  (0.50^0.3 × 0.40^0.4 × 0.85^0.3) ^ 1.0
  = (0.8865 × 0.5743 × 0.9392) ^ 1.0
  = 0.478
  = 47.8

Result: 47.8 / 100
(Notably lower than weighted average: 56.5—geometric mean penalizes the weak 0.40)
```

---

#### 2.8.4 Harmonic Mean

**Formula:**
```
score = 100 × Σ(weight) / Σ(weight / normalized_value)
```

**Why:**
- Most conservative of all methods
- "Worst metric drags down the entire score"
- Used in financial risk analysis
- Highly resistant to outliers, extreme focus on weakest link

**Example (same 3 metrics):**
```
Normalized values: 0.50, 0.40, 0.85

Harmonic mean (weighted):
  100 × (30 + 40 + 30) / (30/0.50 + 40/0.40 + 30/0.85)
  = 100 × 100 / (60 + 100 + 35.29)
  = 100 × 100 / 195.29
  = 51.2

Result: 51.2 / 100
(Even lower—harmonic mean is strictest about weak links)
```

---

#### 2.8.5 Exponential Weighted

**Formula:**
```
exponent = 2 - decay  (where decay ∈ (0, 2))

score = Σ(normalized_value ^ exponent × weight) / Σ(weight) × 100
```

**Why:**
- Penalizes poor performance exponentially
- Amplifies differences between high/low performers
- "Kill switch" for bad metrics
- More aggressive than geometric mean

**Example (same 3 metrics, decay = 1.5, exponent = 2 - 1.5 = 0.5):**
```
Normalized values: 0.50, 0.40, 0.85
Weights: 30, 40, 30
Exponent: 0.5

Exponential weighted:
  (0.50^0.5 × 30 + 0.40^0.5 × 40 + 0.85^0.5 × 30) / 100 × 100
  = (0.707 × 30 + 0.632 × 40 + 0.922 × 30) / 100 × 100
  = (21.21 + 25.28 + 27.66) / 100 × 100
  = 74.15 / 100 × 100
  = 74.15

Result: 74.15 / 100

(Note: Different decay values yield different results:
  decay = 0.5 (exponent = 1.5): Boosts high performers more
  decay = 1.5 (exponent = 0.5): Punishes weak performers more
)
```

---

### 2.9 Normalization Rules

Each metric must be normalized to [0, 1] or [0, 100] scale before scoring.

**For `direction = "higher_is_better"`:**
```
normalized = (value - min) / (max - min)
```

**For `direction = "lower_is_better"`:**
```
normalized = 1 - ((value - min) / (max - min))
OR
normalized = (max - value) / (max - min)
```

**Industry-standard bounds (for common metrics):**

| Metric | Min | Max | Direction |
|--------|-----|-----|-----------|
| profit_factor | 0 | 5 | higher |
| sharpe_ratio | -5 | 5 | higher |
| recovery_factor | -10 | 10 | higher |
| expected_payoff | -0.5 | 1.0 | higher |
| z_score_% | 0 | 100 | higher |
| balance_drawdown_maximal_% | 0 | 100 | lower |
| equity_drawdown_maximal_% | 0 | 100 | lower |
| long_trades_won_% | 0 | 100 | higher |
| short_trades_won_% | 0 | 100 | higher |
| AHPR_% | -100 | 100 | higher |
| GHPR_% | -100 | 100 | higher |

**Rules:**
- If `min_value` is specified, clamp: `value = max(value, min_value)`
- If `cap_value` is specified, clamp: `value = min(value, cap_value)`
- Then apply normalization bounds

---

### 2.10 Config Validation Rules

**Must validate:**

1. **Scoring method** is one of: `weighted_sum`, `weighted_average`, `geometric_mean`, `harmonic_mean`, `exponential_weighted`

2. **Metrics section** `[[metrics]]`:
   - At least 1 metric defined
   - Each metric has: `name`, `weight`, `direction`
   - `name` exists in allowed metrics list
   - `weight` is numeric and >= 0
   - `direction` is `"higher_is_better"` or `"lower_is_better"`
   - `min_value` (if present) is numeric
   - `cap_value` (if present) is numeric
   - `min_value < cap_value` (if both present)

3. **Weight sum**:
   - For `weighted_sum` and `weighted_average`: sum should equal 100.0 (tolerance: ±1.0)
   - For `geometric_mean`, `harmonic_mean`: no requirement (relative weights)
   - For `exponential_weighted`: no requirement

4. **Pass threshold** (if present):
   - Numeric value between 0 and 100

5. **Decay** (only for `exponential_weighted`):
   - Numeric value between 0 (exclusive) and 2 (exclusive)
   - Default: 1.0 if not specified

---

### 2.11 Metrics JSON Validation Rules

When loading metrics JSON:

1. File exists and is valid JSON
2. Root object has `"report(s)"` key
3. `"report(s)"` is an array
4. Each report has:
   - `id`: numeric (integer)
   - `settings`: object with required keys
   - `results`: object with all metric values
5. All metric values in `results`:
   - Are numeric (int or float) or string (for time fields)
   - Are not NaN or Infinity
   - Are not null
6. All required metric keys present in each report's `results`

---

## Part 3: Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
```

---

## Part 4: Error Handling

**Custom error types to define:**

```rust
pub enum MetricsError {
    FileNotFound(String),
    InvalidJson(String),
    MissingMetricKey(String),
    InvalidMetricType { key: String, expected: String, got: String },
    InvalidNumericValue { key: String, value: String },
    HtmlParsingFailed(String),
    AppendFileNotFound(String),
}

pub enum ScoreError {
    FileNotFound(String),
    InvalidJson(String),
    InvalidToml(String),
    ConfigValidationFailed(Vec<String>), // Multiple errors
    MetricsValidationFailed(Vec<String>),
    UnknownMetric(String),
    UnknownScoringMethod(String),
    InvalidWeight(String),
    InvalidDecay(f64),
    CalculationFailed(String),
}
```

---

## Part 5: Output Formatting

**Success messages format:**
```
✓ <action summary>
  <detail line 1>
  <detail line 2>
```

**Failure messages format:**
```
✗ <error summary>
  <category>:
    - <error 1>
    - <error 2>
```

**All paths** should be relative to current working directory for clarity.

---

## Part 6: Implementation Order

Suggested order of implementation:

1. **Metrics extraction** (HTML parsing, validation, JSON output)
2. **Metrics command** with success/failure output
3. **Score config parsing** (TOML)
4. **Score config validation**
5. **Metrics JSON validation** (reuse from metrics command)
6. **Normalization logic** (prepare metrics for scoring)
7. **Weighted average** scoring method
8. **Other scoring methods** (weighted_sum, geometric_mean, harmonic_mean, exponential_weighted)
9. **Score command** with full output
10. **Comprehensive error messages** and edge cases