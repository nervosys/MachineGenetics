// The arena's replay retention, in MAGE.
//
// Plan task 5.2: which entry the pool forgets when it is over capacity is a
// MAGE program the loop can vary, like the reward (plan 5.1) and the budget
// allocation. The kernel keeps what must not vary: the eviction itself, which
// removes exactly one entry, the one with the lowest priority, oldest first
// among ties; and sampling, which stays uniform. The kernel also refuses a
// priority that is not finite, since the eviction could not order it.
//
// `reward` is the entry's compression reward, and `age` is the number of
// rounds since the entry was added, as a float. The shipped priority is the
// reward alone, which is the rule the pool has always used: replay keeps what
// taught the most. A replacement that weighs age in will forget old entries
// sooner. The role allows no effects at all.
@role(candidate)
f priority(reward: f64, age: f64) -> f64 {
    reward
}
