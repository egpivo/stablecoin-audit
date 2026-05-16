fn main() -> anyhow::Result<()> {
    stablecoin_audit::run_cli(std::env::args())
}
