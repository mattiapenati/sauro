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

    pub fn sqrt2(x: f32) -> Result<f32, String> {
        (x > 0.0)
            .then(|| x.sqrt())
            .ok_or_else(|| format!("'{}' is a negative number", x))
    }

    pub fn saxpy(a: f32, x: &[f32], y: &[f32]) -> Box<[f32]> {
        assert_eq!(x.len(), y.len());

        let x = x.iter();
        let y = y.iter();

        x.zip(y).map(|(x, y)| a * x + y).collect()
    }
}
