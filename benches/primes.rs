// primes.rs — native Rust reference for benches/primes.poly.
// Same trial-division algorithm, same output format, so the two are
// directly comparable. Timing uses std::time::Instant.

use std::time::Instant;

fn is_prime(n: i32) -> bool {
    if n < 2 {
        return false;
    }
    if n == 2 {
        return true;
    }
    if n % 2 == 0 {
        return false;
    }
    let mut d = 3;
    while d * d <= n {
        if n % d == 0 {
            return false;
        }
        d += 2;
    }
    true
}

fn main() {
    let start = Instant::now();
    let mut primes: Vec<i32> = Vec::with_capacity(1000);
    let mut candidate: i32 = 2;
    while primes.len() < 1000 {
        if is_prime(candidate) {
            primes.push(candidate);
        }
        candidate += 1;
    }
    let elapsed = start.elapsed();

    println!("First 1000 primes (10 per line):");
    let mut col = 0;
    for p in &primes {
        print!("{:>6}", p);
        col += 1;
        if col == 10 {
            println!();
            col = 0;
        }
    }
    if col != 0 {
        println!();
    }

    println!();
    println!("1000th prime: {}", primes[999]);
    println!("Time taken: {} microseconds", elapsed.as_micros());
}
