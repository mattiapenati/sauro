#[sauro::bridge]
mod deno {
    pub struct Input {
        a: i32,
        b: i32,
    }

    pub fn add(input: Input) -> i32 {
        input.a + input.b
    }
}
