use rand::random;

fn main() {
    let random_val = random::<u8>(); // temporary assignment to a variable to test the debugger.
    println!("Hello, rusty world! Random value is {}", random_val);
}
