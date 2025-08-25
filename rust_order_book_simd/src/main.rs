#![feature(portable_simd)]
#![feature(test)]
extern crate test;

mod order_book;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use order_book::{OrderBookSimd, OrderBook, OrderInfo};

fn create_order_book_simd(pdist: u64, num_orders: usize) -> OrderBookSimd {
    let mut order_book = OrderBookSimd::new(pdist);
    let seed: u64 = 42;
    let mut rng = StdRng::seed_from_u64(seed);
    for p_level in 0..num_orders {
        let volume: f64 = rng.random();
        let price = (p_level as u64 + 1) * (pdist);
        let new_order = OrderInfo::new(price, volume);
        order_book.add_buy_order(new_order);
    }
    return order_book;
}

fn run_order_book_simd(order_book: &OrderBookSimd, vfact: f64){
    let total_vol = order_book.get_total_volume() * vfact;
    let n: usize = 1000;
    let volumes: Vec<f64> = (0..n).map(|i| (i as f64) / (n as f64) * total_vol).collect();
    let mut total_prices: f64 = 0.0;
    for v in volumes.iter() {
        let p_at_v = order_book.get_price_for_volume(v.clone());
        total_prices += p_at_v.unwrap_or_else(|| 0.0f64);
    }
    println!("Total price: {total_prices:?}");
}

#[bench]
fn bench_orderbook_simd(b: &mut test::Bencher) {
    let order_book = create_order_book_simd(100, 5_000);
    b.iter(|| run_order_book_simd(&order_book, 1.0f64));
}

#[bench]
fn bench_orderbook_simd_low(b: &mut test::Bencher) {
    let order_book = create_order_book_simd(100, 5_000);
    b.iter(|| run_order_book_simd(&order_book, 0.05f64));
}


fn create_order_book(pdist: u64, num_orders: usize) -> OrderBook {
    let mut order_book = OrderBook::new();
    let seed: u64 = 42;
    let mut rng = StdRng::seed_from_u64(seed);
    for p_level in 0..num_orders {
        let volume: f64 = rng.random();
        let price = (p_level as u64 + 1) * (pdist);
        let new_order = OrderInfo::new(price, volume);
        order_book.add_buy_order(new_order);
    }
    return order_book;
}

fn run_order_book(order_book: &OrderBook, vfact: f64){
    let total_vol = order_book.get_total_volume()*vfact;
    let n: usize = 1000;
    let volumes: Vec<f64> = (0..n).map(|i| (i as f64) / (n as f64) * total_vol).collect();
    let mut total_prices: f64 = 0.0;
    for v in volumes.iter() {
        let p_at_v = order_book.get_price_for_volume(v.clone());
        total_prices += p_at_v.unwrap_or_else(|| 0.0f64);
    }
    println!("Total price: {total_prices:?}");
}

#[bench]
fn bench_orderbook(b: &mut test::Bencher) {
    let order_book = create_order_book(100, 5_000);
    b.iter(|| run_order_book(&order_book, 1.0f64));
}

#[bench]
fn bench_orderbook_low(b: &mut test::Bencher) {
    let order_book = create_order_book(100, 5_000);
    b.iter(|| run_order_book(&order_book, 0.05f64));
}

fn main() {
    println!("Hello, world!");
}
