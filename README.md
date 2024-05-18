# Tiered-Vec-rs

References:

- [Tiered Vector](https://cs.brown.edu/cgc/jdsl/papers/tiered-vector.pdf)
- [Fast Dynamic Arrays](https://arxiv.org/pdf/1711.00275.pdf)


Optimizations:

- Need only head or tail for internal tiers ()
- optimize read/write by memoizing
- double the number of chunks on expand, try to keep chunk size constant (like cache size)