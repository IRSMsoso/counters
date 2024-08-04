use crate::app::App;
use crate::utils::{init_terminal, restore_terminal};

mod app;
mod utils;

fn main() -> anyhow::Result<()> {
    let terminal = init_terminal()?;

    let mut app = App::default();
    app.run(terminal)?;

    restore_terminal()?;

    Ok(())
}
