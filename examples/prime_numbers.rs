// Generated from Poly source code
#![allow(unused_variables, unused_mut, unused_imports, dead_code)]

fn is_prime(n: i32) -> bool {
    if (n <= 1) {
        return false;
    }

    let mut i: i32 = 2;
    while ((i * i) <= n) {
        if ((n % i) == 0) {
            return false;
        }

        i += 1;
    }
    return true;
}

fn main() {
    println!("{}", "First 20 prime numbers:");
    let mut count: i32 = 0;
    for i in 2..200 {
        if is_prime(i) {
            println!("{}", i);
            count += 1;
            if (count >= 20) {
                break;
            }
        }
    }
    println!(
        "{}",
        "
Done!"
    );
}

