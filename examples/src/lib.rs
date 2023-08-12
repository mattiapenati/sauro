#[sauro::bindgen]
mod deno {
    pub struct Input {
        a: i32,
        b: i32,
    }

    pub fn add(input: Input) -> i32 {
        input.a + input.b
    }

    pub fn add2(a: i32, b: i32) -> i32 {
        a + b
    }

    #[sauro::non_blocking]
    pub fn concat(a: &str, b: &str) -> String {
        format!("{}{}", a, b)
    }

    pub fn sqrt(x: f32) -> Option<f32> {
        (x > 0.0).then(|| x.sqrt())
    }
}
