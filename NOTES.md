# Tiered Vec

A Tiered Vec of size `L` (where `L` is a power of 2) is an intrinsic tree managing `sqrt(L)` Tiers, in which each Tier is a ring buffer of size `sqrt(L)`. Each Tier is a leaf.

For example, in a Tiered Vec of size 4, there will be 2 Tiers, each of size 2.

According to Bille et al.,

> The original L-tiered vector solves the dynamic array problem for L ≥ 2 using Θ(n<sup>1−1/L</sup>)
> extra space while supporting access and update in Θ(L) time and 2L memory probes.
> The operations insert and delete take O(2<sup>L</sup> n<sup>1/L</sup>) time.

An Implicit Tiered Vec avoids the use of pointers by grouping Tier offsets together contiguously. Locating the correct Tier is done trivially, and we can reuse the same index to retrieve its offset information.

Again according to Bille et al.,

> The implicit L-tiered vector solves the dynamic array problem for L ≥ 2 using
> O(n) extra space while supporting access and update in O(L) time requiring L memory probes.
> The operations insert and delete take O(2<sup>L</sup>n<sup>1/L</sup>) time.
