mod output;

fn main() -> anyhow::Result<()> {
    let out = output::Output::from_flags(false, false);
    out.emit(
        &serde_json::json!({"version": env!("CARGO_PKG_VERSION")}),
        |w| {
            use std::io::Write;
            writeln!(w, "yokectl {}", env!("CARGO_PKG_VERSION"))
        },
    )?;
    Ok(())
}
