// The arena's budget allocation, in MAGE.
//
// Plan task 5.2: how an agent's score becomes its claim on the next round's
// budget is a MAGE program the loop can vary, like the reward (plan 5.1).
// The kernel keeps what must not vary: every agent's floor, and rounding the
// weights to whole programs so the shares always sum to the budget. It also
// refuses a weight that is negative or not finite.
//
// `score` is an agent's credit per unit cost, `best` the highest score this
// round (0 when no agent has earned anything) and `temperature` the softmax
// temperature. The weight is a softmax over scores normalised to the best,
// so equal scores get equal shares. It is shifted by the best's term, so the
// best agent's weight is 1 and none overflows at a low temperature (the
// unshifted form gave infinite weights below a temperature of about 0.0014).
// The role allows no effects at all.
@role(candidate)
f weight(score: f64, best: f64, temperature: f64) -> f64 {
    val norm = if best > 0.0 { score / best } else { 0.0 };
    exp((norm - 1.0) / max(temperature, 0.000001))
}
