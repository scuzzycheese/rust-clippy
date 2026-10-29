#![warn(clippy::match_wild_err_arm)]

fn some_call() -> Result<i32, String> {
    Ok(42)
}

// Should lint — not in test context
fn main() {
    if let Ok(res) = some_call() {
        //~^ match_wild_err_arm
        println!("{res}");
    }
}

// Should NOT lint — in a #[test] function (allow-match-wild-err-arm-in-tests = true)
#[test]
fn test_something() {
    if let Ok(res) = some_call() {
        println!("{res}");
    }
}

// Should NOT lint — in a #[cfg(test)] module
#[cfg(test)]
mod tests {
    use super::some_call;

    fn helper() {
        if let Ok(res) = some_call() {
            println!("{res}");
        }
    }
}
