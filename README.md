## Implementation 
In this article we explore the opportunity of applying single
instruction, multiple data (SIMD) vectorisation to accelerate
price calculation algorithms on order book data structures.
Please see <a href="https://github.com/Hoeppke/rust_order_book_simd/tree/main" target="_blank">here</a> for the rust project that this article is based upon.

### Order book structure
During market making, high-frequency trading (HFT), and general
quantitative trading strategies it is important to have an
accurate view of how orders are executed in the order book.

Assuming we have an know the complete state of the order book
at a given time, we can calculate the price at which our orders
will execute given their volume. 

Let us first consider how prices form when a market order
hits the order book. Let us commence by considering the example order book:
| Price   | Volume |
|---------|--------|
| £103.00 | 25     |
| £102.50 | 45     |
| £102.00 | 35     |
| £101.50 | 50     |

We notice a couple of things on this order book representation,
* Prices are quantise (in this example to the nearest
  £0.50), so not every price can be selected when
  submitting an order.
* The orders are sorted in descending (ascending for
 sell orders) order.
Now when we calculate the final execution price, assuming
no further transaction fees, for selling 100 units of our asset,
we observe that we will consume the 25 units available at £103.00,
and the 45 units at £102.50 entirely. The remaining 30 units of our 
order will be filled by partially executing on the order at £102.00.
In summary our order is executed by consuming

| Execution Price | Units |
|-----------------|-------|
| £103.00        | 25    |
| £102.50        | 45    |
| £102.00        | 30    |

for a final execution price of £10,247.50. Immediately after executing
our order the order book will look like

| Price   | Volume |
|---------|--------|
| £102.00 | 5      |
| £101.50 | 50     |

Algorithms to compute execution prices are fundamental to many
trading algorithms, either to exploit arbitrage opportunities
between exchanges, or to execute on a signal predicted by an
alpha engine. As execution speed for such opportunities is key,
we want our pricing algorithm to be as performant as possible.
### A brief introduction to SIMD instructions
Due to cache locality benefits, usign simple vectors in Rust
is often superior compared to more complicated
data structures, such as trees or dictionaries.
We now want to present one further method to speed-up such
price finding algorithms by exploiting single instruction,
multiple data (SIMD) vectorisation capabilities of modern CPUs.
Many modern CPUs contain special registers that are wider than
the standard CPU word size. For example AVX-2 (AVX-256) capable
CPUs contains registers which are 256 bits wide, and thus
capable of storing 4 double precision (64 bit) floating point
numbers.

Once values are loaded in these aligned memory registers we can
execute vectorised operations, allowing us to compute, for
example, four additions using a single CPU cycle. Such instructions
provide the opportunity for parallelism using a single CPU core.

Let us now examine how we can improve the performance of our execution
price algorithm by applying SIMD vectorisation. To achieve this
we will group 4 subsequent orders together in an order block,
allowing us to represent prices and volumes using u64x4 and f64x4 values
respectively.

### Parallel order execution algorithm 
We previously removed an order from our book if its volume was
fully consumed (the remaining volume is 0). Now that we
group orders in sets of four, we drop an entry from our order
vector, if all volumes are zeroed, thus reducing the amount of
memory allocations and de-allocations.

To calculate the execution price on a single block of four orders, 
we first calculate the cumulative volume offered that the four
subsequent prices, this can be done efficiently using only two additions and 
SIMD vector rotations. With this we then calculate the volume
used at each price and, using a final inner product, we calculate
the volume gained and price spend on the current order block.
Here we have profited greatly from Rust's first class support
for SIMD vectorised operations. While this pricing algorithm
appears, at first glance, to be entirely sequential, we see
that it can be vectorised and even help up to reduce
the amount of branches in our code.

### Benchmarks and Rust implementation
The implementation of this project can be found 
<a href="https://github.com/Hoeppke/rust_order_book_simd/tree/main" target="_blank">here</a>.
To benchmark the improvements offered by SIMD vectorised 
order books we compare the standard vector based order book implementation,
to an equivalent one using prices and volumes in u64x4 and f64x4 SIMD  
registers. In both cases we create an order book with depth 5000.
We compare the time required to query prices at volumes equidistantly
placed across the total volume available, and at volumes equidistantly
placed in the lowest 5 per cent of the order book.
When analysing hardware oriented topics, such as SIMD, it is
often interesting to compare their effectiveness across different
processors. For this purpose we compare the performance
benefit on an ARM based MacBook Pro with M2 (12P, 3E) to 
the performance differential on a 12 Core AMD Ryzen 3900x. The
key differentiator between these two for our purpose is that the 
AMD processor offers full support for AVX-2 vectorisation.

| System       | Benchmark   | Baseline Duration | SIMD Duration | SIMD Speed-up |
|-------------|------------|---------------------|---------------|---------------|
| M2 MacBook  | Full Depth | 5.956 s            | 4.481 s       | 1.33×        |
| M2 MacBook  | 5% Depth   | 0.317 s            | 0.212 s       | 1.49×        |
| AMD 3900x   | Full Depth | 7.957 s            | 4.254 s       | 1.87×        |
| AMD 3900x   | 5% Depth   | 0.430 s            | 0.226 s       | 1.90×        |
We observe that both systems benefitted from the SIMD implementation for the same underlying
algorithm and data structure. Additionally, we observe a greater, almost 2x improvement,
when applying the SIMD optimisations on the AMD processor, which is in line with our
expectation, due to the better vectorisation support on this platform.
