#![warn(clippy::match_wild_err_arm)]
#![expect(clippy::match_same_arms)]

fn issue_10635() {
    enum Error {
        A,
        B,
    }

    // Don't trigger in const contexts. Const unwrap is not yet stable
    const X: () = match Ok::<_, Error>(()) {
        Ok(x) => x,
        Err(_) => panic!(),
    };
}

fn match_wild_err_arm() {
    let x: Result<i32, &str> = Ok(3);

    match x {
        Ok(3) => println!("ok"),
        Ok(_) => println!("ok"),
        Err(_) => panic!("err"),
        //~^ match_wild_err_arm
    }

    match x {
        Ok(3) => println!("ok"),
        Ok(_) => println!("ok"),
        Err(_) => panic!(),
        //~^ match_wild_err_arm
    }

    match x {
        Ok(3) => println!("ok"),
        Ok(_) => println!("ok"),
        Err(_) => {
            //~^ match_wild_err_arm

            panic!();
        },
    }

    match x {
        Ok(3) => println!("ok"),
        Ok(_) => println!("ok"),
        Err(_e) => panic!(),
        //~^ match_wild_err_arm
    }

    // Allowed when used in `panic!`.
    match x {
        Ok(3) => println!("ok"),
        Ok(_) => println!("ok"),
        Err(_e) => panic!("{}", _e),
    }

    // Allowed when not with `panic!` block.
    match x {
        Ok(3) => println!("ok"),
        Ok(_) => println!("ok"),
        Err(_) => println!("err"),
    }

    // Allowed when used with `unreachable!`.
    match x {
        Ok(3) => println!("ok"),
        Ok(_) => println!("ok"),
        Err(_) => unreachable!(),
    }

    // Allowed when used with `unreachable!`.
    match x {
        Ok(3) => println!("ok"),
        Ok(_) => println!("ok"),
        Err(_) => {
            unreachable!();
        },
    }
}

fn ignored_result_err() {
    fn some_call() -> Result<i32, String> {
        Ok(42)
    }

    // Should lint — if let with else
    if let Ok(res) = some_call() {
        //~^ match_wild_err_arm
        println!("{res}");
    } else {
        println!("something went wrong");
    }

    // Should lint — if let without else
    if let Ok(res) = some_call() {
        //~^ match_wild_err_arm
        println!("{res}");
    }

    // Should lint — while let
    while let Ok(res) = some_call() {
        //~^ match_wild_err_arm
        println!("{res}");
    }

    // Should lint — let else
    let Ok(res) = some_call() else {
        //~^ match_wild_err_arm
        return;
    };
    println!("{res}");

    // Should NOT lint — match with Err bound
    match some_call() {
        Ok(res) => println!("{res}"),
        Err(e) => println!("failed: {e}"),
    }

    // Should NOT lint — `if let Err(e)` binds the error, it is not discarded
    if let Err(e) = some_call() {
        println!("failed: {e}");
    }

    // Should NOT lint — the error IS bound in the `else if let Err(e)` arm on the
    // same scrutinee (`sc`), so this construct fully handles the error.
    let sc = some_call();
    if let Ok(res) = sc {
        println!("{res}");
    } else if let Err(e) = sc {
        println!("failed: {e}");
    }
}

fn main() {}
