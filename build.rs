use std::process::Command;

fn main() {
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash());
}

fn git_hash() -> String {
    match Command::new("git").args(&["rev-parse", "--short", "HEAD"]).output() {
        Ok(output) => {
            let hash = output.stdout;
            eprintln!("come {}:{}", file!(), line!());

            if hash.is_empty() {
                String::from("unknown")

            } else {
                String::from_utf8(hash)
                    .expect("Git output is not valid UTF-8")
                    .trim()
                    .to_string()
            }
        }

        Err(_) => {
            String::from("unknown")
        }
    }
}
